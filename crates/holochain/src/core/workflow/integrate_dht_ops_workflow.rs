//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::workflow::authored_db_provider::AuthoredDbProvider;
use holo_hash::{AgentPubKey, DhtOpHash, DnaHash};
use holochain_p2p::DynHolochainP2pDna;
use holochain_sqlite::sql::sql_cell::*;
use holochain_state::prelude::*;
use kitsune2_api::StoredOp;
use std::collections::HashMap;
use std::sync::Arc;

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
    authored_db_provider: Arc<dyn AuthoredDbProvider>,
) -> WorkflowResult<WorkComplete> {
    let start = std::time::Instant::now();
    let time = holochain_zome_types::prelude::Timestamp::now();
    let (stored_ops, block_agents, integrated_pairs) = vault
        .write_async(move |txn| {
            let mut stored_ops = Vec::new();
            let mut block_agents = Vec::new();
            let mut integrated_pairs: Vec<(DhtOpHash, Option<AgentPubKey>)> = Vec::new();
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
                    if let Some(ref author_inner) = author {
                        let op_clone_for_block = op_hash.clone();
                        if let Some(warrantee) = warrantee {
                            match validation_status {
                                ValidationStatus::Valid => {
                                    tracing::info!(
                                        ?warrantee,
                                        ?op_hash,
                                        "Warrant op is valid, will block the warrantee"
                                    );
                                    block_agents.push((warrantee, op_clone_for_block.clone()));
                                }
                                ValidationStatus::Rejected => {
                                    tracing::info!(
                                        ?author_inner,
                                        ?op_hash,
                                        "Warrant op is invalid, will block the author"
                                    );
                                    block_agents.push((author_inner.clone(), op_clone_for_block));
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
                    Ok((stored_op, op_hash, author))
                },
            )?;
            for integrated_op in integrated_ops {
                let (stored_op, op_hash, author) = integrated_op?;
                stored_ops.push(stored_op);
                integrated_pairs.push((op_hash, author));
            }

            WorkflowResult::Ok((stored_ops, block_agents, integrated_pairs))
        })
        .await?;
    let changed = stored_ops.len();
    let ops_ps = changed as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
    tracing::info!(?changed, %ops_ps, "ops integrated");
    if changed > 0 {
        let dna_hash = network.dna_hash().clone();
        network.new_integrated_data(stored_ops).await?;

        update_local_authored_status(
            authored_db_provider.as_ref(),
            &dna_hash,
            time,
            integrated_pairs,
        )
        .await?;

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

async fn update_local_authored_status(
    authored_db_provider: &dyn AuthoredDbProvider,
    dna_hash: &DnaHash,
    when_integrated: Timestamp,
    integrated_pairs: Vec<(DhtOpHash, Option<AgentPubKey>)>,
) -> WorkflowResult<()> {
    let mut by_author: HashMap<AgentPubKey, Vec<DhtOpHash>> = HashMap::new();

    for (op_hash, author) in integrated_pairs {
        let Some(author) = author else {
            continue;
        };

        by_author.entry(author).or_default().push(op_hash);
    }

    for (author, op_hashes) in by_author {
        let Some(db) = authored_db_provider
            .get_authored_db(dna_hash, &author)
            .map_err(WorkflowError::from)?
        else {
            continue;
        };

        db.write_async({
            let op_hashes = op_hashes.clone();
            let when_integrated = when_integrated;
            move |txn| -> StateMutationResult<()> {
                for hash in &op_hashes {
                    holochain_state::mutations::set_when_integrated(txn, hash, when_integrated)?;
                }
                Ok(())
            }
        })
        .await?;

        tracing::debug!(
            ?author,
            ?dna_hash,
            ops = ?op_hashes,
            "Marked integrated ops as authored"
        );
    }

    Ok(())
}
