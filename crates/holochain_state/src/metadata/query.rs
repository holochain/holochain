use crate::prelude::test_cell_env;
use fixt::prelude::*;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::rusqlite::TransactionBehavior;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::DhtOpLight;
use holochain_types::EntryHashed;
use holochain_zome_types::fixt::*;
use holochain_zome_types::Entry;
use holochain_zome_types::Header;
use holochain_zome_types::HeaderHashed;
use holochain_zome_types::TryInto;

#[tokio::test(flavor = "multi_thread")]
async fn get_links() {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let mut conn = arc.conn().unwrap();

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

    insert_op(&mut txn, DhtOpHashed::from_content_sync(create_link_op));

    let r = get_link_ops_on_entry(&mut txn, base_hash.clone());
    assert_eq!(
        r.creates[0],
        DhtOpLight::RegisterAddLink(
            HeaderHash::with_data_sync(&Header::CreateLink(create_link.clone())),
            base_hash.clone().into()
        )
    )
}

#[derive(Debug)]
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
                // TODO: Get this from the enum.
                ":create": 7,
                // TODO: Get this from the enum.
                ":entry_hash": entry.into_inner(),
            },
            |row| Ok(from_blob::<_, OpLight>(row.get(row.column_index("Blob")?)?)),
        )
        .unwrap()
        .map(|r| r.unwrap().0)
        .partition(|op| match op {
            DhtOpLight::RegisterAddLink(_, _) => true,
            DhtOpLight::RegisterRemoveLink(_, _) => false,
            _ => panic!("Bad query for link ops"),
        });
    LinkOpsQuery { creates, deletes }
}

// TODO: Just make DhtOpLight derive SerializedBytes if that makes sense.
#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
struct OpLight(pub DhtOpLight);

fn insert_op(txn: &mut Transaction, op: DhtOpHashed) {
    let (op, hash) = op.into_inner();
    let basis = op.dht_basis();
    let op_light = op.to_light();
    let header_hash = op_light.header_hash().clone();
    match op {
        DhtOp::StoreElement(_, _, _) => todo!(),
        DhtOp::StoreEntry(_, _, _) => todo!(),
        DhtOp::RegisterAddLink(_, _) => {
            sql_insert!(txn, DhtOp, {
                "hash": hash.into_inner(),
                // TODO: Get this from the enum.
                "type": 7,
                "basis_hash": basis.into_inner(),
                "header_hash": header_hash.into_inner(),
                "is_authored": 1,
                "is_integrated": 1,
                "require_receipt": 0,
                "blob": to_blob(OpLight(op_light)),
            })
            .unwrap();
        }
        DhtOp::RegisterRemoveLink(_, _) => todo!(),
        _ => todo!(),
    }
}

fn insert_header(txn: &mut Transaction, header: HeaderHashed) {
    let (header, hash) = header.into_inner();
    let header_type = header.header_type();
    let header_seq = header.header_seq();
    match header {
        Header::CreateLink(create_link) => {
            sql_insert!(txn, Header, {
                "hash": hash.into_inner(),
                "type": header_type as i32,
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
                "type": header_type as i32,
                "seq": header_seq,
                "entry_hash": create.entry_hash.clone().into_inner(),
                "blob": to_blob(Header::Create(create)),
            })
            .unwrap();
        }
        _ => todo!(),
    }
}

enum EntryTypeSql {
    Agent,
    App,
    CapClaim,
    CapGrant,
}

fn insert_entry(txn: &mut Transaction, entry: EntryHashed) {
    let (entry, hash) = entry.into_inner();
    sql_insert!(txn, Entry, {
        "hash": hash.into_inner(),
        "type": EntryTypeSql::from(&entry) as i32,
        "blob": to_blob(entry),
    })
    .unwrap();
}

fn to_blob<E: std::fmt::Debug, T: TryInto<SerializedBytes, Error = E>>(t: T) -> Vec<u8> {
    UnsafeBytes::from(t.try_into().unwrap()).into()
}

fn from_blob<E: std::fmt::Debug, T: TryFrom<SerializedBytes, Error = E>>(blob: Vec<u8>) -> T {
    SerializedBytes::from(UnsafeBytes::from(blob))
        .try_into()
        .unwrap()
}

impl From<&Entry> for EntryTypeSql {
    fn from(e: &Entry) -> Self {
        match e {
            Entry::Agent(_) => Self::Agent,
            Entry::App(_) => Self::App,
            Entry::CapClaim(_) => Self::CapClaim,
            Entry::CapGrant(_) => Self::CapGrant,
        }
    }
}
