//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use error::WorkflowResult;
use holochain_p2p::HolochainP2pCell;
use holochain_p2p::HolochainP2pCellT;
use holochain_state::prelude::*;
use holochain_types::prelude::*;

use tracing::*;

#[cfg(test)]
mod query_tests;
#[cfg(feature = "test_utils")]
mod tests;

#[instrument(skip(vault, trigger_receipt, cell_network))]
pub async fn integrate_dht_ops_workflow(
    vault: EnvWrite,
    mut trigger_receipt: TriggerSender,
    cell_network: HolochainP2pCell,
) -> WorkflowResult<WorkComplete> {
    let time = holochain_types::timestamp::now();
    let changed = vault
        .async_commit(move |txn| {
            let changed = txn
                .prepare_cached(holochain_sqlite::sql::sql_cell::UPDATE_INTEGRATE_OPS)?
                .execute(named_params! {
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

                })?;
            WorkflowResult::Ok(changed)
        })
        .await?;
    tracing::debug!(?changed);
    if changed > 0 {
        trigger_receipt.trigger();
        // Check if the ops we just integrated were authored by this cell.
        let authored: bool = fresh_reader!(vault, |txn| {
            DatabaseResult::Ok(txn.query_row(
                "
                SELECT EXISTS(
                    SELECT 1 FROM DhtOp WHERE when_integrated = ? AND is_authored = 1
                )
                ",
                [time],
                |row| row.get(0),
            )?)
        })?;
        cell_network.new_integrated_data(authored).await?;
        Ok(WorkComplete::Incomplete)
    } else {
        Ok(WorkComplete::Complete)
    }
}
