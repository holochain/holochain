use element_details::GetElementDetailsQuery;

use crate::query::entry_details::GetEntryDetailsQuery;

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn entry_scratch_same_as_sql() {
    observability::test_run().ok();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = EntryTestData::new();
    let query = GetEntryDetailsQuery::new(td.hash.clone());
    insert_op_scratch(
        &mut scratch,
        td.store_entry_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();
    insert_op(&mut txn, td.store_entry_op.clone(), true).unwrap();
    set_validation_status(
        &mut txn,
        td.store_entry_op.as_hash().clone(),
        ValidationStatus::Valid,
    )
    .unwrap();
    let r1 = query
        .run(Txn::from(&txn))
        .unwrap()
        .expect("Element not found");
    let r2 = query
        .run(scratch.clone())
        .unwrap()
        .expect("Element not found");
    assert_eq!(r1, r2);
}

#[tokio::test(flavor = "multi_thread")]
async fn element_scratch_same_as_sql() {
    observability::test_run().ok();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = ElementTestData::new();
    let query = GetElementDetailsQuery::new(td.header.as_hash().clone());
    insert_op_scratch(
        &mut scratch,
        td.store_element_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();
    insert_op(&mut txn, td.store_element_op.clone(), true).unwrap();
    set_validation_status(
        &mut txn,
        td.store_element_op.as_hash().clone(),
        ValidationStatus::Valid,
    )
    .unwrap();
    let r1 = query
        .run(Txn::from(&txn))
        .unwrap()
        .expect("Element not found");
    let r2 = query
        .run(scratch.clone())
        .unwrap()
        .expect("Element not found");
    assert_eq!(r1, r2);
}
