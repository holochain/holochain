use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn exists() {
    observability::test_run().ok();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();
    let zome = fixt!(CoordinatorZome);

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = EntryTestData::new();
    insert_op_scratch(
        &mut scratch,
        Some(zome),
        td.store_entry_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();
    insert_op(&mut txn, &td.store_entry_op).unwrap();
    assert!(Txn::from(&txn)
        .contains_hash(&td.hash.clone().into())
        .unwrap());
    assert!(Txn::from(&txn)
        .contains_hash(&td.header.as_hash().clone().into())
        .unwrap());
    assert!(scratch.contains_hash(&td.hash.clone().into()).unwrap());
    assert!(scratch
        .contains_hash(&td.header.as_hash().clone().into())
        .unwrap());
}
