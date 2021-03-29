use ::fixt::prelude::*;
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite::{Transaction, NO_PARAMS};
use holochain_sqlite::{impl_to_sql_via_display, rusqlite::TransactionBehavior};
use holochain_sqlite::{
    rusqlite::{named_params, Connection},
    schema::SCHEMA_CELL,
};
use holochain_types::dht_op::DhtOpLight;
use holochain_types::dht_op::{DhtOpHashed, DhtOpType};
use holochain_types::EntryHashed;
use holochain_types::{dht_op::DhtOp, header::NewEntryHeader};
use holochain_zome_types::*;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;

struct LinkTestData {
    create_link_op: DhtOpHashed,
    delete_link_op: DhtOpHashed,
    link: Link,
    base_op: DhtOpHashed,
    base_hash: EntryHash,
    target_op: DhtOpHashed,
}

struct EntryTestData {
    store_entry_op: DhtOpHashed,
    delete_entry_header_op: DhtOpHashed,
    entry: Entry,
    entry_hash: EntryHash,
    create_hash: HeaderHash,
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

        Self {
            create_link_op: DhtOpHashed::from_content_sync(create_link_op),
            delete_link_op: DhtOpHashed::from_content_sync(delete_link_op),
            link,
            base_op: DhtOpHashed::from_content_sync(base_op),
            base_hash,
            target_op: DhtOpHashed::from_content_sync(target_op),
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

        Self {
            store_entry_op,
            entry_hash,
            create_hash,
            header,
            entry,
            delete_entry_header_op,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_links() {
    observability::test_run().ok();
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
    insert_op(&mut txn, td.base_op.clone());
    insert_op(&mut txn, td.target_op.clone());
    insert_op(&mut txn, td.create_link_op.clone());

    // - Check we can get the link query back.
    let link_ops_query = get_link_query(&mut txn, td.base_hash.clone());
    assert_eq!(*link_ops_query.creates.values().next().unwrap(), td.link);

    // - Check we can resolve this query to a link.
    let r = resolve_links(link_ops_query.clone());
    assert_eq!(r[0], td.link);

    // - Add the same link to the cache.
    insert_op(&mut cache_txn, td.base_op.clone());
    insert_op(&mut cache_txn, td.target_op.clone());
    insert_op(&mut cache_txn, td.create_link_op.clone());

    // - Check duplicates don't cause issues.
    insert_op(&mut cache_txn, td.create_link_op.clone());

    // - Check we can get this query back form the cache.
    let r = get_link_query(&mut cache_txn, td.base_hash.clone());
    assert_eq!(*link_ops_query.creates.values().next().unwrap(), td.link);

    // - Union the both queries.
    let r = r.union(link_ops_query);

    // - Check we can resolve this to a single link.
    let r = resolve_links(r);
    assert_eq!(r[0], td.link);
    assert_eq!(r.len(), 1);

    // - Insert a delete op.
    insert_op(&mut txn, td.delete_link_op.clone());

    // - Get the links from first db.
    let r = get_link_query(&mut txn, td.base_hash.clone());
    // - Union with links from the cache.
    let r = r.union(get_link_query(&mut cache_txn, td.base_hash.clone()));
    // - Resolve the creates / deletes.
    let r = resolve_links(r);
    // - We should not have any links now.
    assert!(r.is_empty())
}

#[tokio::test(flavor = "multi_thread")]
async fn get_entry() {
    observability::test_run().ok();
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
    insert_op(&mut txn, td.store_entry_op.clone());

    // - Check we get that header back.
    let r = get_entry_query(&mut txn, td.entry_hash.clone());
    assert_eq!(*r.creates.keys().next().unwrap(), td.create_hash);

    // - Resolve the query to live headers.
    let r = resolve_entry(r);
    // - Check we get the correct header.
    assert_eq!(r[0], td.header);
    // - Render the element from the live header.
    let r = render_entry(&mut txn, r);
    assert_eq!(*r.entry().as_option().unwrap(), td.entry);

    // - Create the same entry in the cache.
    insert_op(&mut cache_txn, td.store_entry_op.clone());
    // - Check duplicates is ok.
    insert_op(&mut cache_txn, td.store_entry_op.clone());

    // - Get the entry from both stores and union the query results.
    let r = get_entry_query(&mut txn, td.entry_hash.clone());
    let r = r.union(get_entry_query(&mut txn, td.entry_hash.clone()));
    // - Resolve the query to a list of live headers.
    let r = resolve_entry(r);
    // - Check we got the correct header and only one.
    assert_eq!(r[0], td.header);
    assert_eq!(r.len(), 1);
    // - Render the element from the live header.
    let r = render_entry(&mut txn, r);
    // - Check it's the correct entry and header.
    assert_eq!(*r.entry().as_option().unwrap(), td.entry);
    assert_eq!(*r.header(), *td.header.header());

    // - Delete the entry in the cache.
    insert_op(&mut cache_txn, td.delete_entry_header_op.clone());

    // - Get the entry from both stores and union the queries.
    let r = get_entry_query(&mut txn, td.entry_hash.clone());
    let r = r.union(get_entry_query(&mut cache_txn, td.entry_hash.clone()));
    // - Check we got create header.
    assert_eq!(*r.creates.keys().next().unwrap(), td.create_hash);
    // - Check we got the delete for this create header.
    assert_eq!(*r.deletes.iter().next().unwrap(), td.create_hash);
    // - Resolve the entry from the live headers.
    let r = resolve_entry(r);
    // - There should be no live headers so resolving
    // returns an empty list.
    assert!(r.is_empty());
}

// TODO: This could fail if we got the header from a different store.
// Perhaps this should take `&mut [&mut Transaction]` so it can search all
// stores for the entry. Short circuiting on the first place the entry is found.
fn render_entry(txn: &mut Transaction, r: Vec<SignedHeaderHashed>) -> Element {
    // Choose an arbitrary header
    let header = r.into_iter().next().unwrap();
    let entry_hash = header.header().entry_hash().unwrap();
    let entry = txn
        .query_row_named(
            "
            SELECT Entry.blob AS entry_blob FROM Entry
            WHERE hash = :entry_hash
            ",
            named_params! {
                ":entry_hash": entry_hash.clone().into_inner(),
            },
            |row| {
                Ok(from_blob::<Entry>(
                    row.get(row.column_index("entry_blob")?)?,
                ))
            },
        )
        .unwrap();
    Element::new(header, Some(entry))
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
    insert_op_lite(&mut txn1, op.to_light(), op.as_hash().clone());

    // Insert the op in a single step on conn2
    insert_op(&mut txn2, op);

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

#[derive(Debug, PartialEq, Eq, Clone)]
struct LinksQuery {
    creates: HashMap<HeaderHash, Link>,
    deletes: HashSet<HeaderHash>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct EntryQuery {
    creates: HashMap<HeaderHash, SignedHeaderHashed>,
    deletes: HashSet<HeaderHash>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct EntryDetailsQuery {
    creates: HashSet<SignedHeaderHashed>,
    updates: HashSet<SignedHeaderHashed>,
    deletes: HashSet<SignedHeaderHashed>,
}

impl LinksQuery {
    fn union(self, other: Self) -> Self {
        Self {
            creates: self.creates.into_iter().chain(other.creates).collect(),
            deletes: self.deletes.into_iter().chain(other.deletes).collect(),
        }
    }
}

impl EntryQuery {
    fn union(self, other: Self) -> Self {
        Self {
            creates: self.creates.into_iter().chain(other.creates).collect(),
            deletes: self.deletes.into_iter().chain(other.deletes).collect(),
        }
    }
}

impl EntryDetailsQuery {
    fn union(self, other: Self) -> Self {
        Self {
            creates: self.creates.into_iter().chain(other.creates).collect(),
            deletes: self.deletes.into_iter().chain(other.deletes).collect(),
            updates: self.updates.into_iter().chain(other.updates).collect(),
        }
    }
}

macro_rules! sql_insert {
    ($txn:expr, $table:ident, { $($field:literal : $val:expr , )+ $(,)? }) => {{
        let table = stringify!($table);
        let fieldnames = &[ $( { $field } ,)+ ].join(",");
        let fieldvars = &[ $( { format!(":{}", $field) } ,)+ ].join(",");
        let sql = format!("INSERT INTO {} ({}) VALUES ({})", table, fieldnames, fieldvars);
        $txn.execute_named(&sql, &[$(
            (format!(":{}", $field).as_str(), &$val as &dyn holochain_sqlite::rusqlite::ToSql),
        )+])
    }};
}

fn get_link_query(txn: &mut Transaction, entry_hash: EntryHash) -> LinksQuery {
    // We have to make a decision here to either pull out the link data with each op
    // before or after we union and resolve the ops.
    // Doing this before (what we have chosen) may potentially lead to redundant and deleted links
    // being pulled into memory.
    // Doing it after requires a separate query per unique link without a delete.
    let mut stmt = txn
        .prepare(
            "
        SELECT Header.blob AS header_blob FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type = :create
        AND
        DhtOp.basis_hash = :entry_hash
        ",
        )
        .unwrap();
    let creates = stmt
        .query_map_named(
            named_params! {
                ":create": DhtOpType::RegisterAddLink,
                ":entry_hash": entry_hash.clone().into_inner(),
            },
            |row| {
                let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
                let hash = HeaderHash::with_data_sync(&header);
                Ok((hash, header.0))
            },
        )
        .unwrap()
        // TODO: Handle these errors
        .map(Result::unwrap)
        .map(|(hash, header)| (hash, link_from_header(header)))
        .collect();
    let mut stmt = txn
        .prepare(
            "
        SELECT Header.create_link_hash FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type = :delete
        AND
        DhtOp.basis_hash = :entry_hash
        ",
        )
        .unwrap();
    let deletes = stmt
        .query_map_named(
            named_params! {
                ":delete": DhtOpType::RegisterRemoveLink,
                ":entry_hash": entry_hash.into_inner(),
            },
            |row| {
                Ok(
                    HeaderHash::from_raw_39(row.get(row.column_index("create_link_hash")?)?)
                        .unwrap(),
                )
            },
        )
        .unwrap()
        // TODO: Handle these errors
        .map(Result::unwrap)
        .collect();
    LinksQuery { creates, deletes }
}

fn get_entry_query(txn: &mut Transaction, entry_hash: EntryHash) -> EntryQuery {
    let mut stmt = txn
        .prepare(
            "
        SELECT Header.blob AS header_blob FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type = :store_entry
        AND
        DhtOp.basis_hash = :entry_hash
        ",
        )
        .unwrap();
    let creates = stmt
        .query_map_named(
            named_params! {
                ":store_entry": DhtOpType::StoreEntry,
                ":entry_hash": entry_hash.clone().into_inner(),
            },
            |row| {
                let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
                let SignedHeader(header, signature) = header;
                let header = HeaderHashed::from_content_sync(header);
                let hash = header.as_hash().clone();
                let shh = SignedHeaderHashed::with_presigned(header, signature);
                Ok((hash, shh))
            },
        )
        .unwrap()
        // TODO: Handle these errors
        .map(Result::unwrap)
        .collect();
    let mut stmt = txn
        .prepare(
            "
        SELECT Header.deletes_header_hash FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type = :delete
        AND
        DhtOp.basis_hash = :entry_hash
        ",
        )
        .unwrap();
    let deletes = stmt
        .query_map_named(
            named_params! {
                ":delete": DhtOpType::RegisterDeletedEntryHeader,
                ":entry_hash": entry_hash.into_inner(),
            },
            |row| {
                Ok(
                    HeaderHash::from_raw_39(row.get(row.column_index("deletes_header_hash")?)?)
                        .unwrap(),
                )
            },
        )
        .unwrap()
        // TODO: Handle these errors
        .map(Result::unwrap)
        .collect();
    EntryQuery { creates, deletes }
}

fn link_from_header(header: Header) -> Link {
    let hash = HeaderHash::with_data_sync(&header);
    match header {
        Header::CreateLink(header) => Link {
            target: header.target_address,
            timestamp: header.timestamp,
            tag: header.tag,
            create_link_hash: hash,
        },
        _ => panic!("TODO: handle this properly"),
    }
}

fn resolve_links(mut query: LinksQuery) -> Vec<Link> {
    for create_link_address in query.deletes {
        query.creates.remove(&create_link_address);
    }
    query.creates.into_iter().map(|(_, link)| link).collect()
}

fn resolve_entry(mut query: EntryQuery) -> Vec<SignedHeaderHashed> {
    for deletes_header_hash in query.deletes {
        query.creates.remove(&deletes_header_hash);
    }
    query
        .creates
        .into_iter()
        .map(|(_, create)| create)
        .collect()
}

fn insert_op(txn: &mut Transaction, op: DhtOpHashed) {
    let (op, hash) = op.into_inner();
    let op_light = op.to_light();
    let header = op.header();
    let signature = op.signature().clone();
    if let Some(entry) = op.entry() {
        let entry_hashed =
            EntryHashed::with_pre_hashed(entry.clone(), header.entry_hash().unwrap().clone());
        insert_entry(txn, entry_hashed);
    }
    let header_hashed = HeaderHashed::with_pre_hashed(header, op_light.header_hash().to_owned());
    let header_hashed = SignedHeaderHashed::with_presigned(header_hashed, signature);
    insert_header(txn, header_hashed);
    insert_op_lite(txn, op_light, hash);
}

fn insert_op_lite(txn: &mut Transaction, op_lite: DhtOpLight, hash: DhtOpHash) {
    let header_hash = op_lite.header_hash().clone();
    let basis = op_lite.dht_basis().to_owned();
    sql_insert!(txn, DhtOp, {
        "hash": hash.into_inner(),
        "type": op_lite.get_type(),
        "basis_hash": basis.into_inner(),
        "header_hash": header_hash.into_inner(),
        "is_authored": 1,
        "require_receipt": 0,
        "blob": to_blob(op_lite),
    })
    .unwrap();
}

fn insert_header(txn: &mut Transaction, header: SignedHeaderHashed) {
    let (header, signature) = header.into_header_and_signature();
    let (header, hash) = header.into_inner();
    let header_type: HeaderTypeSql = header.header_type().into();
    let header_seq = header.header_seq();
    match header {
        Header::CreateLink(create_link) => {
            sql_insert!(txn, Header, {
                "hash": hash.into_inner(),
                "type": header_type ,
                "seq": header_seq,
                "base_hash": create_link.base_address.clone().into_inner(),
                "zome_id": create_link.zome_id.index() as u32,
                "tag": to_blob(create_link.tag.clone()),
                "blob": to_blob(SignedHeader::from((Header::CreateLink(create_link), signature))),
            })
            .unwrap();
        }
        Header::DeleteLink(delete_link) => {
            sql_insert!(txn, Header, {
                "hash": hash.into_inner(),
                "type": header_type ,
                "seq": header_seq,
                "create_link_hash": delete_link.link_add_address.clone().into_inner(),
                "blob": to_blob(SignedHeader::from((Header::DeleteLink(delete_link), signature))),
            })
            .unwrap();
        }
        Header::Create(create) => {
            sql_insert!(txn, Header, {
                "hash": hash.into_inner(),
                "type": header_type ,
                "seq": header_seq,
                "entry_hash": create.entry_hash.clone().into_inner(),
                "blob": to_blob(SignedHeader::from((Header::Create(create), signature))),
            })
            .unwrap();
        }
        Header::Delete(delete) => {
            sql_insert!(txn, Header, {
                "hash": hash.into_inner(),
                "type": header_type ,
                "seq": header_seq,
                "deletes_entry_hash": delete.deletes_entry_address.clone().into_inner(),
                "deletes_header_hash": delete.deletes_address.clone().into_inner(),
                "blob": to_blob(SignedHeader::from((Header::Delete(delete), signature))),
            })
            .unwrap();
        }
        _ => todo!(),
    }
}

#[derive(Debug, Clone, derive_more::Display, derive_more::From, derive_more::Into)]
pub struct HeaderTypeSql(HeaderType);

impl_to_sql_via_display!(HeaderTypeSql);

/// Just the name of the EntryType
#[derive(derive_more::Display)]
enum EntryTypeName {
    Agent,
    App,
    CapClaim,
    CapGrant,
}

impl_to_sql_via_display!(EntryTypeName);

fn insert_entry(txn: &mut Transaction, entry: EntryHashed) {
    let (entry, hash) = entry.into_inner();
    sql_insert!(txn, Entry, {
        "hash": hash.into_inner(),
        "type": EntryTypeName::from(&entry) ,
        "blob": to_blob(entry),
    })
    .unwrap();
}

fn to_blob<T: Serialize + Debug>(t: T) -> Vec<u8> {
    holochain_serialized_bytes::encode(&t).unwrap()
}

fn from_blob<T: DeserializeOwned + Debug>(blob: Vec<u8>) -> T {
    holochain_serialized_bytes::decode(&blob).unwrap()
}

impl From<&Entry> for EntryTypeName {
    fn from(e: &Entry) -> Self {
        match e {
            Entry::Agent(_) => Self::Agent,
            Entry::App(_) => Self::App,
            Entry::CapClaim(_) => Self::CapClaim,
            Entry::CapGrant(_) => Self::CapGrant,
        }
    }
}
