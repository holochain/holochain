// TODO: This file can probably change a lot. I'm not sure what inserting looks like
// yet and these functions are currently only used in tests. Feel free to change the name.

use crate::query::to_blob;
use crate::scratch::Scratch;
use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::DhtOpLight;
use holochain_types::EntryHashed;
use holochain_zome_types::*;

#[macro_export]
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

pub fn insert_op_scratch(scratch: &mut Scratch, op: DhtOpHashed) {
    let (op, _) = op.into_inner();
    let op_light = op.to_light();
    let header = op.header();
    let signature = op.signature().clone();
    if let Some(entry) = op.entry() {
        let entry_hashed =
            EntryHashed::with_pre_hashed(entry.clone(), header.entry_hash().unwrap().clone());
        scratch.add_entry(entry_hashed);
    }
    let header_hashed = HeaderHashed::with_pre_hashed(header, op_light.header_hash().to_owned());
    let header_hashed = SignedHeaderHashed::with_presigned(header_hashed, signature);
    scratch.add_header(header_hashed);
}

pub fn insert_op(txn: &mut Transaction, op: DhtOpHashed, is_authored: bool) {
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
    insert_op_lite(txn, op_light, hash, is_authored);
}

pub fn insert_op_lite(
    txn: &mut Transaction,
    op_lite: DhtOpLight,
    hash: DhtOpHash,
    is_authored: bool,
) {
    let header_hash = op_lite.header_hash().clone();
    let basis = op_lite.dht_basis().to_owned();
    sql_insert!(txn, DhtOp, {
        "hash": hash,
        "type": op_lite.get_type(),
        "basis_hash": basis,
        "header_hash": header_hash,
        "is_authored": is_authored,
        "require_receipt": 0,
        "blob": to_blob(op_lite),
    })
    .unwrap();
}

pub fn update_op_validation_status(
    txn: &mut Transaction,
    hash: DhtOpHash,
    status: ValidationStatus,
) {
    txn.execute_named(
        "
        UPDATE DhtOp
        SET validation_status = :validation_status
        WHERE hash = :hash
        ",
        named_params! {
            ":validation_status": status,
            ":hash": hash,
        },
    )
    .unwrap();
}

pub fn insert_header(txn: &mut Transaction, header: SignedHeaderHashed) {
    let (header, signature) = header.into_header_and_signature();
    let (header, hash) = header.into_inner();
    let header_type = header.header_type();
    let header_seq = header.header_seq();
    let author = header.author().clone();
    match header {
        Header::CreateLink(create_link) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "author": author,
                "base_hash": create_link.base_address.clone(),
                "zome_id": create_link.zome_id.index() as u32,
                "tag": create_link.tag.clone(),
                "blob": to_blob(SignedHeader::from((Header::CreateLink(create_link), signature))),
            })
            .unwrap();
        }
        Header::DeleteLink(delete_link) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "author": author,
                "create_link_hash": delete_link.link_add_address.clone(),
                "blob": to_blob(SignedHeader::from((Header::DeleteLink(delete_link), signature))),
            })
            .unwrap();
        }
        Header::Create(create) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "author": author,
                "entry_hash": create.entry_hash.clone(),
                "blob": to_blob(SignedHeader::from((Header::Create(create), signature))),
            })
            .unwrap();
        }
        Header::Delete(delete) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "author": author,
                "deletes_entry_hash": delete.deletes_entry_address.clone(),
                "deletes_header_hash": delete.deletes_address.clone(),
                "blob": to_blob(SignedHeader::from((Header::Delete(delete), signature))),
            })
            .unwrap();
        }
        Header::Update(update) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "author": author,
                "original_entry_hash": update.original_entry_address.clone(),
                "original_header_hash": update.original_header_address.clone(),
                "blob": to_blob(SignedHeader::from((Header::Update(update), signature))),
            })
            .unwrap();
        }
        Header::InitZomesComplete(izc) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "author": author,
                "blob": to_blob(SignedHeader::from((Header::InitZomesComplete(izc), signature))),
            })
            .unwrap();
        }
        Header::Dna(dna) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "author": author,
                "blob": to_blob(SignedHeader::from((Header::Dna(dna), signature))),
            })
            .unwrap();
        }
        Header::AgentValidationPkg(avp) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "author": author,
                "blob": to_blob(SignedHeader::from((Header::AgentValidationPkg(avp), signature))),
            })
            .unwrap();
        }
        Header::OpenChain(open) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "author": author,
                "blob": to_blob(SignedHeader::from((Header::OpenChain(open), signature))),
            })
            .unwrap();
        }
        Header::CloseChain(close) => {
            sql_insert!(txn, Header, {
                "hash": hash,
                "type": header_type ,
                "seq": header_seq,
                "author": author,
                "blob": to_blob(SignedHeader::from((Header::CloseChain(close), signature))),
            })
            .unwrap();
        }
    }
}

pub fn insert_entry(txn: &mut Transaction, entry: EntryHashed) {
    let (entry, hash) = entry.into_inner();
    sql_insert!(txn, Entry, {
        "hash": hash,
        "blob": to_blob(entry),
    })
    .unwrap();
}
// /// Just the name of the EntryType
// #[derive(derive_more::Display)]
// pub enum EntryTypeName {
//     Agent,
//     App,
//     CapClaim,
//     CapGrant,
// }

// impl From<&Entry> for EntryTypeName {
//     fn from(e: &Entry) -> Self {
//         match e {
//             Entry::Agent(_) => Self::Agent,
//             Entry::App(_) => Self::App,
//             Entry::CapClaim(_) => Self::CapClaim,
//             Entry::CapGrant(_) => Self::CapGrant,
//         }
//     }
// }

// impl From<&EntryType> for EntryTypeName {
//     fn from(e: &EntryType) -> Self {
//         match e {
//             EntryType::Agent(_) => Self::Agent,
//             EntryType::App(_) => Self::App,
//             EntryType::CapClaim(_) => Self::CapClaim,
//             EntryType::CapGrant(_) => Self::CapGrant,
//         }
//     }
// }
