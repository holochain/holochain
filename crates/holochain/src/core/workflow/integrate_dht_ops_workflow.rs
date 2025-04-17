//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use holochain_p2p::DynHolochainP2pDna;
use holochain_sqlite::sql::sql_cell::*;
use holochain_state::prelude::*;
use kitsune2_api::StoredOp;

#[cfg(test)]
mod tests;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(vault, trigger_receipt, network, dht_query_cache))
)]
pub async fn integrate_dht_ops_workflow(
    vault: DbWrite<DbKindDht>,
    dht_query_cache: DhtDbQueryCache,
    trigger_receipt: TriggerSender,
    network: DynHolochainP2pDna,
) -> WorkflowResult<WorkComplete> {
    let start = std::time::Instant::now();
    let time = holochain_zome_types::prelude::Timestamp::now();
    // Get any activity from the cache that is ready to be integrated.
    let activity_to_integrate = dht_query_cache.get_activity_to_integrate().await?;
    let stored_ops = vault
        .write_async(move |txn| {
            let mut stored_ops = Vec::new();
            let mut stmt = txn.prepare_cached(SET_APP_VALIDATED_OPS_TO_INTEGRATED)?;
            let integrated_ops = stmt.query_map(
                named_params! {
                    ":when_integrated": time,
                },
                to_stored_op,
            )?;
            for integrated_op in integrated_ops {
                stored_ops.push(integrated_op?);
            }

            WorkflowResult::Ok(stored_ops)
        })
        .await?;
    // Once the database transaction is committed, update the cache with the
    // integrated activity.
    dht_query_cache
        .set_all_activity_to_integrated(activity_to_integrate)
        .await?;
    let changed = stored_ops.len();
    let ops_ps = changed as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
    tracing::debug!(?changed, %ops_ps);
    if changed > 0 {
        network.new_integrated_data(stored_ops).await?;
        trigger_receipt.trigger(&"integrate_dht_ops_workflow");
        Ok(WorkComplete::Incomplete(None))
    } else {
        Ok(WorkComplete::Complete)
    }
}

fn to_stored_op(row: &Row<'_>) -> rusqlite::Result<StoredOp> {
    let op_hash = row.get::<_, DhtOpHash>(0)?;
    let created_at = row.get::<_, Timestamp>(1)?;
    let op = StoredOp {
        created_at: kitsune2_api::Timestamp::from_micros(created_at.as_micros()),
        op_id: op_hash.to_k2_op(),
    };
    Ok(op)
}
