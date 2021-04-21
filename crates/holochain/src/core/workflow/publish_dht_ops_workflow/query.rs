use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use holo_hash::AgentPubKey;
use holochain_sqlite::db::ReadManager;
use holochain_state::query::prelude::*;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpType;
use holochain_types::env::EnvRead;
use holochain_zome_types::Entry;
use holochain_zome_types::SignedHeader;
use rusqlite::named_params;

use crate::core::workflow::error::WorkflowResult;

use super::MIN_PUBLISH_INTERVAL;

/// Get all dht ops on an agents chain that need to be published.
/// - Don't publish private entries.
/// - Only get ops that haven't been published within the minimum publish interval
/// - Only get ops that have less then the RECEIPT_BUNDLE_SIZE
pub fn get_ops_to_publish(
    agent: &AgentPubKey,
    env: EnvRead,
    required_receipt_count: u32,
) -> WorkflowResult<Vec<DhtOp>> {
    let earliest_allowed_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|epoch| epoch.checked_sub(MIN_PUBLISH_INTERVAL))
        .map(|t| t.as_secs())
        .unwrap_or(0);
    let results = env.conn()?.with_reader(|txn| {
        let mut stmt = txn.prepare(
            "
            SELECT 
            Header.blob as header_blob,
            Entry.blob as entry_blob,
            DhtOp.type as dht_type
            JOIN
            DhtOp ON DhtOp.header_hash = Header.hash
            LEFT JOIN
            Entry ON (Header.entry_hash IS NULL OR Header.entry_hash = Entry.hash)
            WHERE
            DhtOp.is_authored = 1
            AND
            Header.author = :author
            AND
            Header.private_entry = 0
            AND
            DhtOp.last_publish_time < :earliest_allowed_time
            AND
            DhtOp.receipt_count < :required_receipt_count
            ",
        )?;
        let r = stmt.query_and_then(
            named_params! {
                ":author": agent,
                ":earliest_allowed_time": earliest_allowed_time,
                ":required_receipt_count": required_receipt_count,
            },
            |row| {
                let header = from_blob::<SignedHeader>(row.get("header_blob")?)?;
                let op_type: DhtOpType = row.get("dht_type")?;
                let entry: Option<Vec<u8>> = row.get("entry_blob")?;
                let entry = match entry {
                    Some(entry) => Some(from_blob::<Entry>(entry)?),
                    None => None,
                };
                WorkflowResult::Ok(DhtOp::from_type(op_type, header, entry)?)
            },
        )?;
        WorkflowResult::Ok(r.collect())
    })?;
    results
}
