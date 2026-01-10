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
    let (stored_ops, block_agents) = vault
        .write_async(move |txn| {
            let mut stored_ops = Vec::new();
            let mut block_agents = Vec::new();
            let mut stmt = txn.prepare_cached(SET_VALIDATED_OPS_TO_INTEGRATED)?;
            let integrated_ops = stmt.query_map(
                named_params! {
                    ":when_integrated": time,
                },
                |row| {
                    let op_hash = row.get::<_, DhtOpHash>(0)?;
                    let op_basis = row.get::<_, OpBasis>(1)?;
                    let created_at = row.get::<_, Timestamp>(2)?;
                    let stored_op = StoredOp {
                        created_at: kitsune2_api::Timestamp::from_micros(created_at.as_micros()),
                        op_id: op_hash.to_located_k2_op_id(&op_basis),
                    };
                    let validation_status = row.get::<_, ValidationStatus>(3)?;
                    let author = row.get::<_, Option<AgentPubKey>>(4)?;
                    let warrantee = row.get::<_, Option<AgentPubKey>>(5)?;
                    if let Some(author) = author {
                        if let Some(warrantee) = warrantee {
                            match validation_status {
                                ValidationStatus::Valid => {
                                    tracing::info!(
                                        ?warrantee,
                                        ?op_hash,
                                        "Warrant op is valid, will block the warrantee"
                                    );
                                    block_agents.push((warrantee, op_hash));
                                }
                                ValidationStatus::Rejected => {
                                    tracing::info!(
                                        ?author,
                                        ?op_hash,
                                        "Warrant op is invalid, will block the author"
                                    );
                                    block_agents.push((author, op_hash));
                                }
                                _ => {
                                    tracing::warn!(
                                        ?validation_status,
                                        ?op_hash,
                                        "Unexpected validation status for op being integrated"
                                    );
                                }
                            }
                        }
                    }
                    Ok(stored_op)
                },
            )?;
            for integrated_op in integrated_ops {
                stored_ops.push(integrated_op?);
            }

            WorkflowResult::Ok((stored_ops, block_agents))
        })
        .await?;
    let changed = stored_ops.len();
    let ops_ps = changed as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
    tracing::info!(?changed, %ops_ps, "ops integrated");
    if changed > 0 {
        network.new_integrated_data(stored_ops).await?;

        // Block agents warranted for invalid ops.
        match InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()) {
            Ok(interval) => {
                for (block_agent, invalid_op_hash) in block_agents {
                    // Block agent
                    if let Err(err) = network
                        .block(Block::new(
                            BlockTarget::Cell(
                                CellId::new(network.dna_hash(), block_agent),
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
