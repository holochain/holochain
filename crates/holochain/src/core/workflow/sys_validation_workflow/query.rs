use holo_hash::DhtOpHash;
use holochain_sqlite::db::ReadManager;
use holochain_sqlite::db::WriteManager;
use holochain_state::query::prelude::*;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpType;
use holochain_types::env::EnvRead;
use holochain_types::env::EnvWrite;
use holochain_zome_types::Entry;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::ValidationStatus;
use rusqlite::named_params;

use crate::core::workflow::error::WorkflowResult;

/// Get all ops that need to sys validated in order.
/// - Pending or awaiting sys dependencies.
/// - Ordered by type then timestamp (See [`DhtOpOrder`])
pub fn get_ops_to_sys_validate(env: EnvRead) -> WorkflowResult<Vec<DhtOp>> {
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
            (DhtOp.validation_status IS NULL OR DhtOp.validation_stage = 1)
            ORDER BY 
            CASE DhtOp.type
              WHEN :activity                THEN 0
              WHEN :store_entry             THEN 1
              WHEN :store_element           THEN 2
              WHEN :updated_content         THEN 3
              WHEN :updated_element         THEN 4
              WHEN :deleted_by              THEN 5
              WHEN :deleted_entry_header    THEN 6
              WHEN :add_link                THEN 7
              WHEN :remove_link             THEN 8
            END,
            Header.timestamp_s ASC,
            Header.timestamp_ns ASC
            ",
        )?;
        let r = stmt.query_and_then(
            named_params! {
                ":activity": DhtOpType::RegisterAgentActivity,
                ":store_entry": DhtOpType::StoreEntry,
                ":store_element": DhtOpType::StoreElement,
                ":updated_content": DhtOpType::RegisterUpdatedContent,
                ":updated_element": DhtOpType::RegisterUpdatedElement,
                ":deleted_by": DhtOpType::RegisterDeletedBy,
                ":deleted_entry_header": DhtOpType::RegisterDeletedEntryHeader,
                ":add_link": DhtOpType::RegisterAddLink,
                ":remove_link": DhtOpType::RegisterRemoveLink,
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

fn put_int_limbo(env: EnvWrite, hash: DhtOpHash, status: ValidationStatus) -> WorkflowResult<()> {
    env.conn()?.with_commit(|txn| {
        txn.execute(
            "
            UPDATE DhtOp
            SET 
            validation_status = :status,
            validation_stage = NULL,
            WHERE hash = :hash
            ",
            named_params! {
                ":status": status,
                ":hash": hash,
            },
        )?;
        WorkflowResult::Ok(())
    })
}
