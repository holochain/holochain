use std::{path::Path, sync::Arc};

use contrafact::arbitrary;
use contrafact::arbitrary::Arbitrary;
use holo_hash::DnaHash;
use holochain_sqlite::rusqlite::Connection;
use holochain_state::prelude::{
    fresh_reader_test, mutations_helpers, DbKindAuthored, DbKindCache, DbKindT,
};
use holochain_types::{
    db::DbWrite,
    dht_op::{DhtOp, DhtOpHashed},
};
use holochain_zome_types::{Action, Signature};
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread")]
/// Checks a corrupt cache will be wiped on load.
async fn corrupt_cache_creates_new_db() {
    let mut u = arbitrary::Unstructured::new(&holochain_zome_types::NOISE);
    observability::test_run().ok();

    let kind = DbKindCache(Arc::new(DnaHash::arbitrary(&mut u).unwrap()));

    // - Create a corrupt cache db.
    let testdir = create_corrupt_db(kind.clone(), &mut u);

    // - Try to open it.
    let db = DbWrite::test(testdir.path(), kind).unwrap();

    // - It opens successfully but the data is wiped.
    let n: usize = fresh_reader_test(db, |txn| {
        txn.query_row("SELECT COUNT(rowid) FROM DhtOp", [], |row| row.get(0))
            .unwrap()
    });
    assert_eq!(n, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn corrupt_source_chain_panics() {
    let mut u = arbitrary::Unstructured::new(&holochain_zome_types::NOISE);
    observability::test_run().ok();

    let kind = DbKindAuthored(Arc::new(DnaHash::arbitrary(&mut u).unwrap()));

    // - Create a corrupt cell db.
    let testdir = create_corrupt_db(kind.clone(), &mut u);

    // - Try to open it.
    let result = DbWrite::test(testdir.path(), kind);

    // - It cannot open.
    assert!(result.is_err());
}

/// Corrupts some bytes of the db.
fn corrupt_db(path: &Path) {
    let mut file = std::fs::read(path).unwrap();

    for (i, b) in file.iter_mut().take(200).enumerate() {
        if i % 2 == 0 {
            *b = 0;
        }
    }
    std::fs::write(path, file).unwrap();
}

/// Creates a db with some data in it then corrupts the db.
fn create_corrupt_db<Kind: DbKindT>(kind: Kind, u: &mut arbitrary::Unstructured) -> TempDir {
    let testdir = tempfile::Builder::new()
        .prefix("corrupt_source_chain")
        .tempdir()
        .unwrap();
    let path = testdir.path().join(kind.filename());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut conn = Connection::open(&path).unwrap();
    holochain_sqlite::schema::SCHEMA_CELL
        .initialize(&mut conn, Some(kind.kind()))
        .unwrap();
    let op = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
        Signature::arbitrary(u).unwrap(),
        Action::arbitrary(u).unwrap(),
    ));
    let mut txn = conn
        .transaction_with_behavior(holochain_sqlite::rusqlite::TransactionBehavior::Exclusive)
        .unwrap();
    mutations_helpers::insert_valid_integrated_op(&mut txn, &op).unwrap();
    txn.commit().unwrap();
    conn.close().unwrap();
    corrupt_db(path.as_ref());
    testdir
}
