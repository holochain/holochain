use ::fixt::*;
use holochain_sqlite::rusqlite::Connection;
use holochain_state::prelude::{mutations_helpers, *};
use std::{path::Path, sync::Arc};
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread")]
async fn corrupt_source_chain_panics() {
    holochain_trace::test_run();

    let kind = DbKindAuthored(Arc::new(fixt!(CellId)));

    // - Create a corrupt cell db.
    let testdir = create_corrupt_db(kind.clone());

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
fn create_corrupt_db<Kind: DbKindT>(kind: Kind) -> TempDir {
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
    let op = DhtOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(
        Signature(vec![1; 64].try_into().unwrap()),
        Action::Create(fixt!(Create)),
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
