use crate::scratch::Scratch;
use ::fixt::prelude::*;
use holo_hash::*;
use holochain_sqlite::rusqlite::TransactionBehavior;
use holochain_sqlite::rusqlite::{Transaction, NO_PARAMS};
use holochain_sqlite::{rusqlite::Connection, schema::SCHEMA_CELL};
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::EntryHashed;
use holochain_types::{dht_op::DhtOp, header::NewEntryHeader};
use holochain_zome_types::*;

use super::entry::*;
use super::link::*;
use super::*;
use crate::insert::*;

struct LinkTestData {
    create_link_op: DhtOpHashed,
    delete_link_op: DhtOpHashed,
    link: Link,
    base_op: DhtOpHashed,
    target_op: DhtOpHashed,
    base_query: LinkQuery,
    tag_query: LinkQuery,
}

struct EntryTestData {
    store_entry_op: DhtOpHashed,
    delete_entry_header_op: DhtOpHashed,
    entry: Entry,
    query: GetEntryQuery,
    header: SignedHeaderHashed,
}

impl LinkTestData {
    fn new() -> Self {
        let mut create_link = fixt!(CreateLink);
        let mut delete_link = fixt!(DeleteLink);

        let mut create_base = fixt!(Create);
        let base = fixt!(Entry);
        let base_hash = EntryHash::with_data_sync(&base);
        create_base.entry_hash = base_hash.clone();

        let mut create_target = fixt!(Create);
        let target = fixt!(Entry);
        let target_hash = EntryHash::with_data_sync(&target);
        create_target.entry_hash = target_hash.clone();

        create_link.base_address = base_hash.clone();
        create_link.target_address = target_hash.clone();

        let create_link_sig = fixt!(Signature);
        let create_link_op = DhtOp::RegisterAddLink(create_link_sig.clone(), create_link.clone());

        let create_link_hash = HeaderHash::with_data_sync(&Header::CreateLink(create_link.clone()));

        delete_link.link_add_address = create_link_hash.clone();
        delete_link.base_address = base_hash.clone();

        let delete_link_op = DhtOp::RegisterRemoveLink(fixt!(Signature), delete_link.clone());

        let base_op = DhtOp::StoreEntry(
            fixt!(Signature),
            NewEntryHeader::Create(create_base.clone()),
            Box::new(base.clone()),
        );

        let target_op = DhtOp::StoreEntry(
            fixt!(Signature),
            NewEntryHeader::Create(create_target.clone()),
            Box::new(target.clone()),
        );

        let link = Link {
            target: target_hash.clone(),
            timestamp: create_link.timestamp.clone(),
            tag: create_link.tag.clone(),
            create_link_hash: create_link_hash.clone(),
        };

        let base_query = LinkQuery::base(base_hash.clone(), create_link.zome_id.clone());
        let tag_query = LinkQuery::tag(
            base_hash.clone(),
            create_link.zome_id.clone(),
            create_link.tag.clone(),
        );

        Self {
            create_link_op: DhtOpHashed::from_content_sync(create_link_op),
            delete_link_op: DhtOpHashed::from_content_sync(delete_link_op),
            link,
            base_op: DhtOpHashed::from_content_sync(base_op),
            target_op: DhtOpHashed::from_content_sync(target_op),
            base_query,
            tag_query,
        }
    }
}

impl EntryTestData {
    fn new() -> Self {
        let mut create = fixt!(Create);
        let mut delete = fixt!(Delete);
        let entry = fixt!(Entry);
        let entry_hash = EntryHash::with_data_sync(&entry);
        create.entry_hash = entry_hash.clone();

        let create_hash = HeaderHash::with_data_sync(&Header::Create(create.clone()));

        delete.deletes_entry_address = entry_hash.clone();
        delete.deletes_address = create_hash.clone();

        let signature = fixt!(Signature);
        let store_entry_op = DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
            signature.clone(),
            NewEntryHeader::Create(create.clone()),
            Box::new(entry.clone()),
        ));

        let header = SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::Create(create.clone())),
            signature.clone(),
        );

        let signature = fixt!(Signature);
        let delete_entry_header_op = DhtOpHashed::from_content_sync(
            DhtOp::RegisterDeletedEntryHeader(signature.clone(), delete.clone()),
        );

        let query = GetEntryQuery::new(entry_hash.clone());

        Self {
            store_entry_op,
            header,
            entry,
            query,
            delete_entry_header_op,
        }
    }
}

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
    insert_op(&mut txn, td.base_op.clone(), false);
    insert_op(&mut txn, td.target_op.clone(), false);
    insert_op(&mut txn, td.create_link_op.clone(), false);

    // - Check we can get the link query back.
    let r = get_link_query(&mut [&mut txn], None, td.tag_query.clone());
    assert_eq!(r[0], td.link);

    // - Add the same link to the cache.
    insert_op(&mut cache_txn, td.base_op.clone(), false);
    insert_op(&mut cache_txn, td.target_op.clone(), false);
    insert_op(&mut cache_txn, td.create_link_op.clone(), false);

    // - Check duplicates don't cause issues.
    insert_op(&mut cache_txn, td.create_link_op.clone(), false);

    // - Add to the scratch
    insert_op_scratch(&mut scratch, td.base_op.clone());
    insert_op_scratch(&mut scratch, td.target_op.clone());
    insert_op_scratch(&mut scratch, td.create_link_op.clone());

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
    insert_op(&mut txn, td.delete_link_op.clone(), false);

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
    insert_op(&mut txn, td.store_entry_op.clone(), false);

    // - Check we get that header back.
    let r = get_entry_query(&mut [&mut txn], None, td.query.clone()).unwrap();
    assert_eq!(*r.entry().as_option().unwrap(), td.entry);

    // - Create the same entry in the cache.
    insert_op(&mut cache_txn, td.store_entry_op.clone(), false);
    // - Check duplicates is ok.
    insert_op(&mut cache_txn, td.store_entry_op.clone(), false);

    // - Add to the scratch
    insert_op_scratch(&mut scratch, td.store_entry_op.clone());

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
    insert_op(&mut cache_txn, td.delete_entry_header_op.clone(), false);

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

    insert_entry(&mut txn1, EntryHashed::from_content_sync(create_entry));
    insert_header(
        &mut txn1,
        SignedHeaderHashed::with_presigned(
            HeaderHashed::from_content_sync(Header::Create(create_header.clone())),
            fixt!(Signature),
        ),
    );
    insert_op_lite(&mut txn1, op.to_light(), op.as_hash().clone(), false);

    // Insert the op in a single step on conn2
    insert_op(&mut txn2, op, false);

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
    query: LinkQuery,
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
    query: GetEntryQuery,
) -> Option<Element> {
    match scratch {
        Some(scratch) => {
            let stores = DbScratch::new(txns, scratch);
            query.run(stores).unwrap()
        }
        None => query.run(Txns::from(txns)).unwrap(),
    }
}
