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
use holochain_zome_types::Entry;
use holochain_zome_types::Header;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::{fixt::*, HeaderType};
use serde::de::DeserializeOwned;
use std::fmt::Debug;

#[tokio::test(flavor = "multi_thread")]
async fn get_links() {
    observability::test_run().ok();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    // Add link to db
    let mut create_link = fixt!(CreateLink);

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

    let sig = fixt!(Signature);
    let create_link_op = DhtOp::RegisterAddLink(sig.clone(), create_link.clone());
    let (op, op_hash) = DhtOpHashed::from_content_sync(create_link_op).into_inner();
    let op_lite = op.to_light();
    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    insert_header(
        &mut txn,
        HeaderHashed::from_content_sync(Header::CreateLink(create_link.clone())),
    );
    insert_entry(&mut txn, EntryHashed::from_content_sync(base));
    insert_header(
        &mut txn,
        HeaderHashed::from_content_sync(Header::Create(create_base.clone())),
    );
    insert_entry(&mut txn, EntryHashed::from_content_sync(target));
    insert_header(
        &mut txn,
        HeaderHashed::from_content_sync(Header::Create(create_target.clone())),
    );

    insert_op_lite(&mut txn, op_lite, op_hash);

    let r = get_link_ops_on_entry(&mut txn, base_hash.clone());
    assert_eq!(
        r.creates[0],
        DhtOpLight::RegisterAddLink(
            HeaderHash::with_data_sync(&Header::CreateLink(create_link.clone())),
            base_hash.clone().into()
        )
    )
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
        HeaderHashed::from_content_sync(Header::Create(create_header.clone())),
    );
    insert_op_lite(&mut txn1, op.to_light(), op.as_hash().clone());

    // Insert the op in a single step on conn2
    insert_op(&mut txn2, op);

    drop(txn1);
    drop(txn2);

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

#[derive(Debug, PartialEq, Eq)]
struct LinkOpsQuery {
    creates: Vec<DhtOpLight>,
    deletes: Vec<DhtOpLight>,
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

fn get_link_ops_on_entry(txn: &mut Transaction, entry: EntryHash) -> LinkOpsQuery {
    let mut stmt = txn
        .prepare(
            "
        SELECT DhtOp.Blob FROM DhtOp
        WHERE DhtOp.type IN (:create, :delete)
        AND
        DhtOp.basis_hash = :entry_hash
        ",
        )
        .unwrap();
    let (creates, deletes) = stmt
        .query_map_named(
            named_params! {
                ":create": DhtOpType::RegisterAddLink,
                ":delete": DhtOpType::RegisterRemoveLink,
                ":entry_hash": entry.into_inner(),
            },
            |row| Ok(from_blob::<DhtOpLight>(row.get(row.column_index("Blob")?)?)),
        )
        .unwrap()
        .map(|r| r.unwrap())
        .partition(|op| match op {
            DhtOpLight::RegisterAddLink(_, _) => true,
            DhtOpLight::RegisterRemoveLink(_, _) => false,
            _ => panic!("Bad query for link ops"),
        });
    LinkOpsQuery { creates, deletes }
}

fn insert_op(txn: &mut Transaction, op: DhtOpHashed) {
    let (op, hash) = op.into_inner();
    let op_light = op.to_light();
    let header = op.header();
    if let Some(entry) = op.entry() {
        let entry_hashed =
            EntryHashed::with_pre_hashed(entry.clone(), header.entry_hash().unwrap().clone());
        insert_entry(txn, entry_hashed);
    }
    let header_hashed = HeaderHashed::with_pre_hashed(header, op_light.header_hash().to_owned());
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

fn insert_header(txn: &mut Transaction, header: HeaderHashed) {
    let (header, hash) = header.into_inner();
    let header_type: HeaderTypeSql = header.header_type().into();
    let header_seq = header.header_seq();
    match header {
        Header::CreateLink(create_link) => {
            sql_insert!(txn, Header, {
                "hash": hash.into_inner(),
                "type": header_type ,
                "seq": header_seq,
                "basis_hash": create_link.base_address.clone().into_inner(),
                "blob": to_blob(Header::CreateLink(create_link)),
            })
            .unwrap();
        }
        Header::DeleteLink(_) => todo!(),
        Header::Create(create) => {
            sql_insert!(txn, Header, {
                "hash": hash.into_inner(),
                "type": header_type ,
                "seq": header_seq,
                "entry_hash": create.entry_hash.clone().into_inner(),
                "blob": to_blob(Header::Create(create)),
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
