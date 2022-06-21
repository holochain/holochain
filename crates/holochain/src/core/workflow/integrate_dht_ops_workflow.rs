//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use error::WorkflowResult;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::prelude::*;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::prelude::*;

use tracing::*;

#[cfg(test)]
mod query_tests;
#[cfg(feature = "test_utils")]
mod tests;

#[instrument(skip(vault, trigger_receipt, network, dht_query_cache))]
pub async fn integrate_dht_ops_workflow(
    vault: DbWrite<DbKindDht>,
    dht_query_cache: &DhtDbQueryCache,
    trigger_receipt: TriggerSender,
    network: HolochainP2pDna,
) -> WorkflowResult<WorkComplete> {
    let start = std::time::Instant::now();
    let time = holochain_zome_types::Timestamp::now();
    // Get any activity from the cache that is ready to be integrated.
    let activity_to_integrate = dht_query_cache.get_activity_to_integrate().await?;
    let (changed, activity_integrated) = vault
        .async_commit(move |txn| {
            let mut total = 0;
            if !activity_to_integrate.is_empty() {
                let mut stmt = txn.prepare_cached(
                    holochain_sqlite::sql::sql_cell::UPDATE_INTEGRATE_DEP_ACTIVITY,
                )?;
                for (author, seq_range) in &activity_to_integrate {
                    let start = seq_range.start();
                    let end = seq_range.end();
                    total += stmt.execute(named_params! {
                        ":when_integrated": time,
                        ":register_activity": DhtOpType::RegisterAgentActivity,
                        ":seq_start": start,
                        ":seq_end": end,
                        ":author": author,
                    })?;
                }
            }
            let changed = txn
                .prepare_cached(holochain_sqlite::sql::sql_cell::UPDATE_INTEGRATE_DEP_STORE_ENTRY)?
                .execute(named_params! {
                    ":when_integrated": time,
                    ":updated_content": DhtOpType::RegisterUpdatedContent,
                    ":deleted_entry_action": DhtOpType::RegisterDeletedEntryAction,
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
                .prepare_cached(holochain_sqlite::sql::sql_cell::UPDATE_INTEGRATE_DEP_STORE_RECORD)?
                .execute(named_params! {
                    ":when_integrated": time,
                    ":store_commit": DhtOpType::StoreCommit,
                    ":updated_commit": DhtOpType::RegisterUpdatedCommit,
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
            WorkflowResult::Ok((total, activity_to_integrate))
        })
        .await?;
    // Once the database transaction is committed, update the cache with the
    // integrated activity.
    dht_query_cache
        .set_all_activity_to_integrated(activity_integrated)
        .await?;
    let ops_ps = changed as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
    tracing::debug!(?changed, %ops_ps);
    if changed > 0 {
        trigger_receipt.trigger(&"integrate_dht_ops_workflow");
        network.new_integrated_data().await?;
        Ok(WorkComplete::Incomplete)
    } else {
        Ok(WorkComplete::Complete)
    }
}
