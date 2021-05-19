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
    let mut conn = vault.conn()?;
    let changed = conn.with_commit(|txn| {
        let changed = txn
            .prepare_cached(holochain_sqlite::UPDATE_INTEGRATE_OPS)?
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
