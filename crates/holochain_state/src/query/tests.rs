use crate::scratch::Scratch;
use ::fixt::prelude::*;
use holo_hash::*;
use holochain_sqlite::rusqlite::TransactionBehavior;
use holochain_sqlite::rusqlite::{Transaction, NO_PARAMS};
use holochain_sqlite::{rusqlite::Connection, schema::SCHEMA_CELL};
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::OpOrder;
use holochain_types::EntryHashed;
use holochain_types::{dht_op::DhtOp, header::NewEntryHeader};
use holochain_zome_types::*;

use super::link::*;
use super::live_entry::*;
use super::test_data::*;
use super::*;
use crate::mutations::*;

#[cfg(todo_redo_old_tests)]
mod chain_sequence;
#[cfg(todo_redo_old_tests)]
mod chain_test;
mod details;
mod links;
#[cfg(todo_redo_old_tests)]
mod links_test;
mod store;
#[cfg(todo_redo_old_tests)]
mod sys_meta;

#[tokio::test(flavor = "multi_thread")]
async fn get_links() {
    observability::test_run().ok();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut cache = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut cache, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let mut cache_txn = cache
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = LinkTestData::new();

    // - Add link to db.
    insert_op(&mut txn, td.base_op.clone(), false).unwrap();
    insert_op(&mut txn, td.target_op.clone(), false).unwrap();
    insert_op(&mut txn, td.create_link_op.clone(), false).unwrap();

    // - Check we can get the link query back.
    let r = get_link_query(&mut [&mut txn], None, td.tag_query.clone());
    assert_eq!(r[0], td.link);

    // - Add the same link to the cache.
    insert_op(&mut cache_txn, td.base_op.clone(), false).unwrap();
    insert_op(&mut cache_txn, td.target_op.clone(), false).unwrap();
    insert_op(&mut cache_txn, td.create_link_op.clone(), false).unwrap();

    // - Check duplicates don't cause issues.
    insert_op(&mut cache_txn, td.create_link_op.clone(), false).unwrap();

    // - Add to the scratch
    insert_op_scratch(&mut scratch, td.base_op.clone()).unwrap();
    insert_op_scratch(&mut scratch, td.target_op.clone()).unwrap();
    insert_op_scratch(&mut scratch, td.create_link_op.clone()).unwrap();

    // - Check we can resolve this to a single link.
    let r = get_link_query(&mut [&mut cache_txn], Some(&scratch), td.base_query.clone());
    assert_eq!(r[0], td.link);
    assert_eq!(r.len(), 1);
    let r = get_link_query(
        &mut [&mut cache_txn, &mut txn],
        Some(&scratch),
        td.tag_query.clone(),
    );
    assert_eq!(r[0], td.link);
    assert_eq!(r.len(), 1);

    // - Insert a delete op.
    insert_op(&mut txn, td.delete_link_op.clone(), false).unwrap();

    let r = get_link_query(
        &mut [&mut cache_txn, &mut txn],
        Some(&scratch),
        td.tag_query.clone(),
    );
    // - We should not have any links now.
    assert!(r.is_empty())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_entry() {
    observability::test_run().ok();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut cache = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut cache, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let mut cache_txn = cache
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = EntryTestData::new();

    // - Create an entry on main db.
    insert_op(&mut txn, td.store_entry_op.clone(), false).unwrap();

    // - Check we get that header back.
    let r = get_entry_query(&mut [&mut txn], None, td.query.clone()).unwrap();
    assert_eq!(*r.entry().as_option().unwrap(), td.entry);

    // - Create the same entry in the cache.
    insert_op(&mut cache_txn, td.store_entry_op.clone(), false).unwrap();
    // - Check duplicates is ok.
    insert_op(&mut cache_txn, td.store_entry_op.clone(), false).unwrap();

    // - Add to the scratch
    insert_op_scratch(&mut scratch, td.store_entry_op.clone()).unwrap();

    // - Get the entry from both stores and union the query results.
    let r = get_entry_query(
        &mut [&mut txn, &mut cache_txn],
        Some(&scratch),
        td.query.clone(),
    );
    // - Check it's the correct entry and header.
    let r = r.unwrap();
    assert_eq!(*r.entry().as_option().unwrap(), td.entry);
    assert_eq!(*r.header(), *td.header.header());

    // - Delete the entry in the cache.
    insert_op(&mut cache_txn, td.delete_entry_header_op.clone(), false).unwrap();

    // - Get the entry from both stores and union the queries.
    let r = get_entry_query(
        &mut [&mut txn, &mut cache_txn],
        Some(&scratch),
        td.query.clone(),
    );
    // - There should be no live headers so resolving
    // returns no element.
    assert!(r.is_none());
}

/// Test that `insert_op` also inserts a header and potentially an entry
#[tokio::test(flavor = "multi_thread")]
async fn insert_op_equivalence() {
    observability::test_run().ok();
    let mut conn1 = Connection::open_in_memory().unwrap();
    let mut conn2 = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn1, None).unwrap();
    SCHEMA_CELL.initialize(&mut conn2, None).unwrap();

    let mut create_header = fixt!(Create);
    let create_entry = fixt!(Entry);
    let create_entry_hash = EntryHash::with_data_sync(&create_entry);
    create_header.entry_hash = create_entry_hash.clone();

    let sig = fixt!(Signature);
    let op = DhtOp::StoreEntry(
        sig.clone(),
        NewEntryHeader::Create(create_header.clone()),
        Box::new(create_entry.clone()),
    );
    let op = DhtOpHashed::from_content_sync(op);

    // Insert the op in 3 steps on conn1
    let mut txn1 = conn1
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let mut txn2 = conn2
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    insert_entry(&mut txn1, EntryHashed::from_content_sync(create_entry)).unwrap();
    let op_order = OpOrder::new(op.get_type(), create_header.timestamp);
    insert_header(
        &mut txn1,
        SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::Create(create_header.clone())),
            fixt!(Signature),
        ),
    )
    .unwrap();
    insert_op_lite(
        &mut txn1,
        op.to_light(),
        op.as_hash().clone(),
        false,
        op_order,
    )
    .unwrap();

    // Insert the op in a single step on conn2
    insert_op(&mut txn2, op, false).unwrap();

    txn1.commit().unwrap();
    txn2.commit().unwrap();

    // Query the DB on conn1
    let entries1: Vec<u8> = conn1
        .query_row("SELECT * FROM Entry", NO_PARAMS, |row| row.get("hash"))
        .unwrap();
    let headers1: Vec<u8> = conn1
        .query_row("SELECT * FROM Header", NO_PARAMS, |row| row.get("hash"))
        .unwrap();
    let ops1: Vec<u8> = conn1
        .query_row("SELECT * FROM DhtOp", NO_PARAMS, |row| row.get("hash"))
        .unwrap();

    // Query the DB on conn2
    let entries2: Vec<u8> = conn2
        .query_row("SELECT * FROM Entry", NO_PARAMS, |row| row.get("hash"))
        .unwrap();
    let headers2: Vec<u8> = conn2
        .query_row("SELECT * FROM Header", NO_PARAMS, |row| row.get("hash"))
        .unwrap();
    let ops2: Vec<u8> = conn2
        .query_row("SELECT * FROM DhtOp", NO_PARAMS, |row| row.get("hash"))
        .unwrap();

    assert_eq!(entries1, entries2);
    assert_eq!(headers1, headers2);
    assert_eq!(ops1, ops2);
}

fn get_link_query<'a, 'b: 'a>(
    txns: &[&'a Transaction<'b>],
    scratch: Option<&Scratch>,
    query: GetLinksQuery,
) -> Vec<Link> {
    match scratch {
        Some(scratch) => {
            let stores = DbScratch::new(txns, scratch);
            query.run(stores).unwrap()
        }
        None => query.run(Txns::from(txns)).unwrap(),
    }
}

fn get_entry_query<'a, 'b: 'a>(
    txns: &[&'a Transaction<'b>],
    scratch: Option<&Scratch>,
    query: GetLiveEntryQuery,
) -> Option<Element> {
    match scratch {
        Some(scratch) => {
            let stores = DbScratch::new(txns, scratch);
            query.run(stores).unwrap()
        }
        None => query.run(Txns::from(txns)).unwrap(),
    }
}
