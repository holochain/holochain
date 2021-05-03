//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use error::WorkflowResult;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::prelude::*;

use tracing::*;

#[cfg(test)]
mod query_tests;
#[cfg(feature = "test_utils")]
mod tests;

#[instrument(skip(vault, trigger_sys, trigger_receipt))]
pub async fn integrate_dht_ops_workflow(
    vault: EnvWrite,
    mut trigger_sys: TriggerSender,
    mut trigger_receipt: TriggerSender,
) -> WorkflowResult<WorkComplete> {
    let time = holochain_types::timestamp::now();
    let changed = vault.conn()?.with_commit(|txn| {
        let dep = "
            SELECT 1 FROM Header AS H_DEP
            JOIN DhtOp AS OP_DEP ON OP_DEP.header_hash = H_DEP.hash 
            WHERE 
            OP_DEP.when_integrated IS NOT NULL
        ";
        let activity = format!(
            "{}
            AND Header.prev_hash = H_DEP.hash
            AND OP_DEP.type = :register_activity
            ",
            dep
        );
        let update_content = format!(
            "{}
			AND Header.original_header_hash = H_DEP.hash
			AND OP_DEP.type = :store_entry 
            ",
            dep
        );
        let update_element = format!(
            "{}
			AND Header.original_header_hash = H_DEP.hash
			AND OP_DEP.type = :store_element
            ",
            dep
        );
        let deleted_entry_header = format!(
            "{}
			AND Header.deletes_header_hash = H_DEP.hash
			AND OP_DEP.type = :store_entry
            ",
            dep
        );
        let deleted_by = format!(
            "{}
			AND Header.deletes_header_hash = H_DEP.hash
			AND OP_DEP.type = :store_element
            ",
            dep
        );
        let create_link = format!(
            "{}
			AND Header.base_hash = H_DEP.entry_hash
			AND OP_DEP.type = :store_entry
            ",
            dep
        );
        let delete_link = format!(
            "{}
			AND Header.create_link_hash = H_DEP.hash
			AND OP_DEP.type = :create_link
            ",
            dep
        );
        let ops = format!(
            "
            CASE DhtOp.type
                WHEN :store_entry               THEN 1
                WHEN :store_element             THEN 1
                WHEN :register_activity         THEN EXISTS({activity})
                WHEN :updated_content           THEN EXISTS({update_content})
                WHEN :updated_element           THEN EXISTS({update_element})
                WHEN :deleted_by                THEN EXISTS({deleted_by})
                WHEN :deleted_entry_header      THEN EXISTS({deleted_entry_header})
                WHEN :create_link               THEN EXISTS({create_link})
                WHEN :delete_link               THEN EXISTS({delete_link})
            END
            ",
            activity = activity,
            update_content = update_content,
            update_element = update_element,
            deleted_by = deleted_by,
            deleted_entry_header = deleted_entry_header,
            create_link = create_link,
            delete_link = delete_link,
        );
        let sql = format!(
            "
            UPDATE DhtOp
            SET
            when_integrated = :when_integrated,
            when_integrated_ns = :when_integrated_ns,
            validation_stage = NULL
            WHERE 
            validation_stage = 3
            AND
            DhtOp.header_hash IN (
                SELECT Header.hash
                FROM Header
                WHERE
                {}
            )
            ",
            ops
        );
        let mut stmt = txn.prepare(&sql)?;

        let changed = stmt.execute(
            // &sql,
            named_params! {
                ":when_integrated": time,
                ":when_integrated_ns": to_blob(time)?,
                ":store_entry": DhtOpType::StoreEntry,
                ":store_element": DhtOpType::StoreElement,
                ":register_activity": DhtOpType::RegisterAgentActivity,
                ":updated_content": DhtOpType::RegisterUpdatedContent,
                ":updated_element": DhtOpType::RegisterUpdatedElement,
                ":deleted_by": DhtOpType::RegisterDeletedBy,
                ":deleted_entry_header": DhtOpType::RegisterDeletedEntryHeader,
                ":create_link": DhtOpType::RegisterAddLink,
                ":delete_link": DhtOpType::RegisterRemoveLink,

            },
        )?;
        tracing::debug!("{}", stmt.expanded_sql().unwrap());
        WorkflowResult::Ok(changed)
    })?;
    tracing::debug!(?changed);
    if changed > 0 {
        trigger_sys.trigger();
        trigger_receipt.trigger();
        Ok(WorkComplete::Incomplete)
    } else {
        Ok(WorkComplete::Complete)
    }
}
