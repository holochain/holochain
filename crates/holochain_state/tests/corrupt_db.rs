use std::path::Path;

use contrafact::arbitrary;
use contrafact::arbitrary::Arbitrary;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_sqlite::rusqlite::Connection;
use holochain_state::prelude::{fresh_reader_test, mutations_helpers, test_keystore, DbKind};
use holochain_types::{
    dht_op::{DhtOp, DhtOpHashed},
    env::DbWrite,
};
use holochain_zome_types::{CellId, Header, Signature};
use tempdir::TempDir;

#[tokio::test(flavor = "multi_thread")]
/// Checks a corrupt cache will be wiped on load.
async fn corrupt_cache_creates_new_db() {
    let mut u = arbitrary::Unstructured::new(&holochain_zome_types::NOISE);
    observability::test_run().ok();

    let kind = DbKind::Cache(DnaHash::arbitrary(&mut u).unwrap());

    // - Create a corrupt cache db.
    let testdir = create_corrupt_db(&kind, &mut u);

    // - Try to open it.
    let env = DbWrite::test(&testdir, kind, test_keystore()).unwrap();

    // - It opens successfully but the data is wiped.
    let n: usize = fresh_reader_test(env, |txn| {
        txn.query_row("SELECT COUNT(rowid) FROM DhtOp", [], |row| row.get(0))
            .unwrap()
    });
    assert_eq!(n, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn corrupt_source_chain_panics() {
    let mut u = arbitrary::Unstructured::new(&holochain_zome_types::NOISE);
    observability::test_run().ok();

    let kind = DbKind::Authored(DnaHash::arbitrary(&mut u).unwrap());

    // - Create a corrupt cell db.
    let testdir = create_corrupt_db(&kind, &mut u);

    // - Try to open it.
    let result = DbWrite::test(&testdir, kind, test_keystore());

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
fn create_corrupt_db(kind: &DbKind, u: &mut arbitrary::Unstructured) -> TempDir {
    let testdir = tempdir::TempDir::new("corrupt_source_chain").unwrap();
    let path = testdir.path().join(kind.filename());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut conn = Connection::open(&path).unwrap();
    holochain_sqlite::schema::SCHEMA_CELL
        .initialize(&mut conn, Some(&kind))
        .unwrap();
    let op = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
        Signature::arbitrary(u).unwrap(),
        Header::arbitrary(u).unwrap(),
    ));
    let mut txn = conn
        .transaction_with_behavior(holochain_sqlite::rusqlite::TransactionBehavior::Exclusive)
        .unwrap();
    mutations_helpers::insert_valid_integrated_op(&mut txn, op).unwrap();
    txn.commit().unwrap();
    conn.close().unwrap();
    corrupt_db(path.as_ref());
    testdir
}
