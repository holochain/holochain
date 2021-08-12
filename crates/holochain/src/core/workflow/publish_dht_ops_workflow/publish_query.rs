// use std::time::SystemTime;
// use std::time::UNIX_EPOCH;

use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holochain_state::query::prelude::*;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::DhtOpType;
use holochain_types::env::EnvRead;
use holochain_zome_types::Entry;
use holochain_zome_types::EntryVisibility;
use holochain_zome_types::SignedHeader;
use rusqlite::named_params;

use crate::core::workflow::error::WorkflowResult;

// use super::MIN_PUBLISH_INTERVAL;

/// Get all dht ops on an agents chain that need to be published.
/// - Don't publish private entries.
/// - Only get ops that haven't been published within the minimum publish interval
/// - Only get ops that have less then the RECEIPT_BUNDLE_SIZE
pub async fn get_ops_to_publish(
    agent: AgentPubKey,
    env: &EnvRead,
    _required_receipt_count: u32,
) -> WorkflowResult<Vec<DhtOpHashed>> {
    // let earliest_allowed_time = SystemTime::now()
    //     .duration_since(UNIX_EPOCH)
    //     .ok()
    //     .and_then(|epoch| epoch.checked_sub(MIN_PUBLISH_INTERVAL))
    //     .map(|t| t.as_secs())
    //     .unwrap_or(0);

    let results = env
        .async_reader(move |txn| {
            let mut stmt = txn.prepare(
                "
            SELECT 
            Header.blob as header_blob,
            Entry.blob as entry_blob,
            DhtOp.type as dht_type,
            DhtOp.hash as dht_hash
            FROM Header
            JOIN
            DhtOp ON DhtOp.header_hash = Header.hash
            LEFT JOIN
            Entry ON Header.entry_hash = Entry.hash
            WHERE
            DhtOp.is_authored = 1
            AND
            Header.author = :author
            AND
            (DhtOp.type != :store_entry OR Header.private_entry = 0)
            AND
            DhtOp.last_publish_time IS NULL
            ",
                // (DhtOp.last_publish_time IS NULL OR DhtOp.last_publish_time <= :earliest_allowed_time)
                // AND
                // (DhtOp.receipt_count IS NULL OR DhtOp.receipt_count < :required_receipt_count)
            )?;
            let r = stmt.query_and_then(
                named_params! {
                    ":author": agent,
                    // ":earliest_allowed_time": earliest_allowed_time,
                    // ":required_receipt_count": required_receipt_count,
                    ":store_entry": DhtOpType::StoreEntry,
                },
                |row| {
                    let header = from_blob::<SignedHeader>(row.get("header_blob")?)?;
                    let op_type: DhtOpType = row.get("dht_type")?;
                    let hash: DhtOpHash = row.get("dht_hash")?;
                    let entry = match header.0.entry_type().map(|et| et.visibility()) {
                        Some(EntryVisibility::Public) => {
                            let entry: Option<Vec<u8>> = row.get("entry_blob")?;
                            match entry {
                                Some(entry) => Some(from_blob::<Entry>(entry)?),
                                None => None,
                            }
                        }
                        _ => None,
                    };
                    WorkflowResult::Ok(DhtOpHashed::with_pre_hashed(
                        DhtOp::from_type(op_type, header, entry)?,
                        hash,
                    ))
                },
            )?;
            WorkflowResult::Ok(r.collect())
        })
        .await?;
    tracing::debug!(?results);
    results
}

// #[cfg(test)]
// mod tests {
//     use fixt::prelude::*;
//     use holo_hash::EntryHash;
//     use holo_hash::HasHash;
//     use holochain_sqlite::db::WriteManager;
//     use holochain_sqlite::prelude::DatabaseResult;
//     use holochain_state::prelude::insert_op;
//     use holochain_state::prelude::set_last_publish_time;
//     use holochain_state::prelude::set_receipt_count;
//     use holochain_state::prelude::test_cell_env;
//     use holochain_types::dht_op::DhtOpHashed;
//     use holochain_types::header::NewEntryHeader;
//     use holochain_zome_types::fixt::*;
//     use holochain_zome_types::EntryType;
//     use holochain_zome_types::EntryVisibility;
//     use holochain_zome_types::Header;

//     use crate::core::workflow::publish_dht_ops_workflow::DEFAULT_RECEIPT_BUNDLE_SIZE;

//     use super::*;

//     #[derive(Debug, Clone, Copy)]
//     struct Facts {
//         private: bool,
//         within_min_period: bool,
//         has_required_receipts: bool,
//         is_this_agent: bool,
//         is_authored: bool,
//         store_entry: bool,
//     }

//     struct Consistent {
//         this_agent: AgentPubKey,
//         required_receipt_count: u32,
//     }

//     struct Expected {
//         agent: AgentPubKey,
//         required_receipt_count: u32,
//         results: Vec<DhtOpHashed>,
//     }

//     #[tokio::test(flavor = "multi_thread")]
//     async fn publish_query() {
//         observability::test_run().ok();
//         let env = test_cell_env();
//         let expected = test_data(&env.env().into());
//         let r = get_ops_to_publish(
//             &expected.agent,
//             &env.env().into(),
//             expected.required_receipt_count,
//         )
//         .unwrap();
//         assert_eq!(r, expected.results);
//     }

//     fn create_and_insert_op(
//         env: &EnvRead,
//         facts: Facts,
//         consistent_data: &Consistent,
//     ) -> DhtOpHashed {
//         let this_agent = consistent_data.this_agent.clone();
//         let entry = Entry::App(fixt!(AppEntryBytes));
//         let mut header = fixt!(Create);
//         header.author = this_agent.clone();
//         header.entry_hash = EntryHash::with_data_sync(&entry);
//         if facts.private {
//             // - Private: true
//             header.entry_type = AppEntryTypeFixturator::new(EntryVisibility::Private)
//                 .map(EntryType::App)
//                 .next()
//                 .unwrap();
//         } else {
//             // - Private: false
//             header.entry_type = AppEntryTypeFixturator::new(EntryVisibility::Public)
//                 .map(EntryType::App)
//                 .next()
//                 .unwrap();
//         }

//         // - IsThisAgent: false.
//         if !facts.is_this_agent {
//             header.author = fixt!(AgentPubKey);
//         }

//         let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
//         let last_publish = if facts.within_min_period {
//             // - WithinMinPeriod: true.
//             now
//         } else {
//             // - WithinMinPeriod: false.
//             now - MIN_PUBLISH_INTERVAL
//         };

//         let required_receipt_count = if facts.has_required_receipts {
//             // - HasRequireReceipts: true.
//             consistent_data.required_receipt_count
//         } else {
//             // - HasRequireReceipts: false.
//             0
//         };

//         let state = if facts.store_entry {
//             DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
//                 fixt!(Signature),
//                 NewEntryHeader::Create(header.clone()),
//                 Box::new(entry.clone()),
//             ))
//         } else {
//             DhtOpHashed::from_content_sync(DhtOp::StoreElement(
//                 fixt!(Signature),
//                 Header::Create(header.clone()),
//                 Some(Box::new(entry.clone())),
//             ))
//         };

//         env.conn()
//             .unwrap()
//             .with_commit(|txn| {
//                 let hash = state.as_hash().clone();
//                 insert_op(txn, state.clone(), facts.is_authored).unwrap();
//                 set_last_publish_time(txn, hash.clone(), last_publish).unwrap();
//                 set_receipt_count(txn, hash, required_receipt_count).unwrap();
//                 DatabaseResult::Ok(())
//             })
//             .unwrap();
//         state
//     }

//     fn test_data(env: &EnvRead) -> Expected {
//         let mut results = Vec::new();
//         let cd = Consistent {
//             this_agent: fixt!(AgentPubKey),
//             required_receipt_count: DEFAULT_RECEIPT_BUNDLE_SIZE,
//         };
//         // We **do** expect any of these in the results:
//         // - Private: false.
//         // - WithinMinPeriod: false.
//         // - HasRequireReceipts: false.
//         // - IsThisAgent: true.
//         // - IsAuthored: true.
//         // - StoreEntry: true
//         let facts = Facts {
//             private: false,
//             within_min_period: false,
//             has_required_receipts: false,
//             is_this_agent: true,
//             is_authored: true,
//             store_entry: true,
//         };
//         let op = create_and_insert_op(env, facts, &cd);
//         results.push(op);

//         // All facts are the same unless stated:

//         // - Private: true.
//         // - StoreEntry: false.
//         let mut f = facts;
//         f.private = true;
//         f.store_entry = false;
//         let op = create_and_insert_op(env, f, &cd);
//         results.push(op);

//         // We **don't** expect any of these in the results:
//         // - Private: true.
//         let mut f = facts;
//         f.private = true;
//         create_and_insert_op(env, f, &cd);

//         // - WithinMinPeriod: true.
//         let mut f = facts;
//         f.within_min_period = true;
//         create_and_insert_op(env, f, &cd);

//         // - HasRequireReceipts: true.
//         let mut f = facts;
//         f.has_required_receipts = true;
//         create_and_insert_op(env, f, &cd);

//         // - IsThisAgent: false.
//         let mut f = facts;
//         f.is_this_agent = false;
//         create_and_insert_op(env, f, &cd);

//         // - IsAuthored: false.
//         let mut f = facts;
//         f.is_authored = false;
//         create_and_insert_op(env, f, &cd);

//         Expected {
//             agent: cd.this_agent.clone(),
//             required_receipt_count: cd.required_receipt_count,
//             results,
//         }
//     }
// }
