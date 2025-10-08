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
    tracing::instrument(skip(vault, trigger_receipt, network))
)]
pub async fn integrate_dht_ops_workflow(
    vault: DbWrite<DbKindDht>,
    trigger_receipt: TriggerSender,
    network: DynHolochainP2pDna,
) -> WorkflowResult<WorkComplete> {
    let start = std::time::Instant::now();
    let time = holochain_zome_types::prelude::Timestamp::now();
    let (stored_ops, warrantees) = vault
        .write_async(move |txn| {
            let mut stored_ops = Vec::new();
            let mut warrantees = Vec::new();
            let mut stmt = txn.prepare_cached(SET_VALIDATED_OPS_TO_INTEGRATED)?;
            let integrated_ops = stmt.query_map(
                named_params! {
                    ":when_integrated": time,
                },
                |row| {
                    let op_hash = row.get::<_, DhtOpHash>(0)?;
                    let op_basis = row.get::<_, OpBasis>(1)?;
                    let op_type: DhtOpType = row.get(2)?;
                    let created_at = row.get::<_, Timestamp>(3)?;
                    let stored_op = StoredOp {
                        created_at: kitsune2_api::Timestamp::from_micros(created_at.as_micros()),
                        op_id: op_hash.to_located_k2_op_id(&op_basis),
                    };
                    if matches!(op_type, DhtOpType::Warrant(_)) {
                        // The op basis of a warrant op is the warrantee's agent pub key.
                        if let Some(warrantee) = op_basis.clone().into_agent_pub_key() {
                            warrantees.push((warrantee, op_hash));
                        } else {
                            tracing::error!(
                                ?op_basis,
                                "expected basis hash of warrant op to be an agent pub key"
                            );
                        }
                    }
                    Ok(stored_op)
                },
            )?;
            for integrated_op in integrated_ops {
                stored_ops.push(integrated_op?);
            }

            WorkflowResult::Ok((stored_ops, warrantees))
        })
        .await?;
    let changed = stored_ops.len();
    let ops_ps = changed as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
    tracing::debug!(?changed, %ops_ps);
    if changed > 0 {
        network.new_integrated_data(stored_ops).await?;

        // Block agents warranted for invalid ops.
        match InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()) {
            Ok(interval) => {
                for (warrantee, invalid_op_hash) in warrantees {
                    // Block agent
                    if let Err(err) = network
                        .block(Block::new(
                            BlockTarget::Cell(
                                CellId::new(network.dna_hash(), warrantee),
                                CellBlockReason::InvalidOp(invalid_op_hash),
                            ),
                            interval.clone(),
                        ))
                        .await
                    {
                        tracing::warn!(?err, "Error blocking agent");
                    }
                }
            }
            Err(err) => {
                tracing::error!(?err, "Invalid interval when blocking agents")
            }
        }

        trigger_receipt.trigger(&"integrate_dht_ops_workflow");
        Ok(WorkComplete::Incomplete(None))
    } else {
        Ok(WorkComplete::Complete)
    }
}
