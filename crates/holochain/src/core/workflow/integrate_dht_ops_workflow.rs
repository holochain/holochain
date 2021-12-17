//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use error::WorkflowResult;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::prelude::*;
use holochain_types::prelude::*;

use tracing::*;

#[cfg(test)]
mod query_tests;
#[cfg(feature = "test_utils")]
mod tests;

#[instrument(skip(vault, trigger_receipt, network))]
pub async fn integrate_dht_ops_workflow(
    vault: DbWrite<DbKindDht>,
    trigger_receipt: TriggerSender,
    network: HolochainP2pDna,
) -> WorkflowResult<WorkComplete> {
    let start = std::time::Instant::now();
    let time = holochain_zome_types::Timestamp::now();
    let changed = vault
        .async_commit(move |txn| {
            let mut total = 0;
            let changed = txn
                .prepare_cached(holochain_sqlite::sql::sql_cell::UPDATE_INTEGRATE_DEP_ACTIVITY)?
                .execute(named_params! {
                    ":when_integrated": time,
                    ":register_activity": DhtOpType::RegisterAgentActivity,
                })?;
            total += changed;
            let changed = txn
                .prepare_cached(holochain_sqlite::sql::sql_cell::UPDATE_INTEGRATE_DEP_STORE_ENTRY)?
                .execute(named_params! {
                    ":when_integrated": time,
                    ":updated_content": DhtOpType::RegisterUpdatedContent,
                    ":deleted_entry_header": DhtOpType::RegisterDeletedEntryHeader,
                    ":store_entry": DhtOpType::StoreEntry,
                })?;
            total += changed;
            let changed = txn
                .prepare_cached(
                    holochain_sqlite::sql::sql_cell::UPDATE_INTEGRATE_DEP_STORE_ENTRY_BASIS,
                )?
                .execute(named_params! {
                    ":when_integrated": time,
                    ":create_link": DhtOpType::RegisterAddLink,
                    ":store_entry": DhtOpType::StoreEntry,
                })?;
            total += changed;
            let changed = txn
                .prepare_cached(
                    holochain_sqlite::sql::sql_cell::UPDATE_INTEGRATE_DEP_STORE_ELEMENT,
                )?
                .execute(named_params! {
                    ":when_integrated": time,
                    ":store_element": DhtOpType::StoreElement,
                    ":updated_element": DhtOpType::RegisterUpdatedElement,
                    ":deleted_by": DhtOpType::RegisterDeletedBy,
                })?;
            total += changed;
            let changed = txn
                .prepare_cached(holochain_sqlite::sql::sql_cell::UPDATE_INTEGRATE_DEP_CREATE_LINK)?
                .execute(named_params! {
                    ":when_integrated": time,
                    ":create_link": DhtOpType::RegisterAddLink,
                    ":delete_link": DhtOpType::RegisterRemoveLink,

                })?;
            total += changed;
            WorkflowResult::Ok(total)
        })
        .await?;
    let ops_ps = changed as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
    tracing::debug!(?changed, %ops_ps);
    if changed > 0 {
        trigger_receipt.trigger();
        network.new_integrated_data().await?;
        Ok(WorkComplete::Incomplete)
    } else {
        Ok(WorkComplete::Complete)
    }
}
