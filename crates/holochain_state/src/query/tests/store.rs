use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn exists() {
    holochain_trace::test_run();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = EntryTestData::new();
    insert_op_scratch(
        &mut scratch,
        td.store_entry_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();
    insert_op_unchecked(&mut txn, &td.store_entry_op.downcast()).unwrap();
    assert!(Txn::from(&txn)
        .contains_hash(&td.hash.clone().into())
        .unwrap());
    assert!(Txn::from(&txn)
        .contains_hash(&td.action.as_hash().clone().into())
        .unwrap());
    assert!(scratch.contains_hash(&td.hash.clone().into()).unwrap());
    assert!(scratch
        .contains_hash(&td.action.as_hash().clone().into())
        .unwrap());
}
