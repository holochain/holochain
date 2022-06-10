use holochain_sqlite::rusqlite::Connection;
use holochain_sqlite::rusqlite::TransactionBehavior;
use holochain_sqlite::schema::SCHEMA_CELL;

use crate::mutations::insert_op_scratch;
use crate::prelude::mutations_helpers::insert_valid_integrated_op;
use crate::query::test_data::EntryTestData;

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn can_handle_update_in_scratch() {
    observability::test_run().ok();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = EntryTestData::new();

    // - Create an entry on main db.
    insert_valid_integrated_op(&mut txn, &td.update_store_entry_op).unwrap();
    let r = td
        .query
        .run(Txn::from(&txn))
        .unwrap()
        .expect("Element not found");
    assert_eq!(*r.entry().as_option().unwrap(), td.entry);
    assert_eq!(*r.header(), *td.update_header.header());

    // - Add to the scratch
    insert_op_scratch(
        &mut scratch,
        td.update_store_entry_op.clone(),
        ChainTopOrdering::default(),
    )
    .unwrap();
    let r = td
        .query
        .run(scratch.clone())
        .unwrap()
        .expect("Element not found");
    assert_eq!(*r.entry().as_option().unwrap(), td.entry);
    assert_eq!(*r.header(), *td.update_header.header());
}
