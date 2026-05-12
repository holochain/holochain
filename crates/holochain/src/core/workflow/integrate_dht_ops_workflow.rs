//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::metrics::{
    op_integration_delay_metric, op_validation_attempts_metric, workflow_integrated_op_metric,
};
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use holo_hash::{AgentPubKey, DhtOpHash, DnaHash};
use holochain_p2p::DynHolochainP2pDna;
use holochain_sqlite::sql::sql_cell::*;
use holochain_state::dht_store::DhtStore;
use holochain_state::prelude::*;
use kitsune2_api::StoredOp;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(
        vault,
        dht_store,
        trigger_receipt,
        network,
        authored_db_provider,
        publish_trigger_provider
    ))
)]
pub async fn integrate_dht_ops_workflow(
    vault: DbWrite<DbKindDht>,
    dht_store: DhtStore,
    trigger_receipt: TriggerSender,
    network: DynHolochainP2pDna,
    authored_db_provider: Arc<dyn super::provider::authored_db_provider::AuthoredDbProvider>,
    publish_trigger_provider: Arc<
        dyn super::provider::publish_trigger_provider::PublishTriggerProvider,
    >,
) -> WorkflowResult<WorkComplete> {
    let start = std::time::Instant::now();
    let when_integrated = Timestamp::now();
    let (stored_ops, block_agents, integrated_pairs, when_stored_times, validation_attempts) =
        vault
            .write_async(move |txn| {
                let mut stored_ops = Vec::new();
                let mut block_agents = Vec::new();
                let mut integrated_pairs: Vec<(DhtOpHash, Option<AgentPubKey>)> = Vec::new();
                let mut when_stored_times: Vec<Option<Timestamp>> = Vec::new();
                let mut validation_attempts: Vec<Option<u32>> = Vec::new();
                let mut stmt = txn.prepare_cached(SET_VALIDATED_OPS_TO_INTEGRATED)?;
                let integrated_ops = stmt.query_map(
                    named_params! {
                        ":when_integrated": when_integrated,
                    },
                    |row| {
                        let op_hash = row.get::<_, DhtOpHash>(0)?;
                        let op_basis = row.get::<_, OpBasis>(1)?;
                        let created_at = row.get::<_, Timestamp>(2)?;
                        let stored_op = StoredOp {
                            created_at: kitsune2_api::Timestamp::from_micros(
                                created_at.as_micros(),
                            ),
                            op_id: op_hash.to_located_k2_op_id(&op_basis),
                        };
                        let validation_status = row.get::<_, ValidationStatus>(3)?;
                        let when_stored = row.get::<_, Option<Timestamp>>(4)?;
                        let num_validation_attempts = row.get::<_, Option<u32>>(5)?;
                        let action_author = row.get::<_, Option<AgentPubKey>>(6)?;
                        let warrant_author = row.get::<_, Option<AgentPubKey>>(7)?;
                        let warrantee = row.get::<_, Option<AgentPubKey>>(8)?;
                        if let Some(ref warrantee) = warrantee {
                            let Some(ref warrant_author) = warrant_author else {
                                tracing::warn!(
                                    ?op_hash,
                                    "Warrant missing author while integrating"
                                );
                                return Ok((
                                    stored_op,
                                    op_hash,
                                    action_author,
                                    when_stored,
                                    num_validation_attempts,
                                ));
                            };

                            let op_clone_for_block = op_hash.clone();
                            match validation_status {
                                ValidationStatus::Valid => {
                                    tracing::info!(
                                        ?warrantee,
                                        ?op_hash,
                                        "Warrant op is valid, will block the warrantee"
                                    );
                                    block_agents.push((warrantee.clone(), op_clone_for_block));
                                }
                                ValidationStatus::Rejected => {
                                    tracing::info!(
                                        ?warrant_author,
                                        ?op_hash,
                                        "Warrant op is invalid, will block the author"
                                    );
                                    block_agents.push((warrant_author.clone(), op_clone_for_block));
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

                        Ok((
                            stored_op,
                            op_hash,
                            action_author,
                            when_stored,
                            num_validation_attempts,
                        ))
                    },
                )?;
                for integrated_op in integrated_ops {
                    let (stored_op, op_hash, author, when_stored, num_attempts) = integrated_op?;
                    stored_ops.push(stored_op);
                    integrated_pairs.push((op_hash, author));
                    when_stored_times.push(when_stored);
                    validation_attempts.push(num_attempts);
                }

                WorkflowResult::Ok((
                    stored_ops,
                    block_agents,
                    integrated_pairs,
                    when_stored_times,
                    validation_attempts,
                ))
            })
            .await?;
    let _new_promoted = dht_store
        .integrate_ready_ops(when_integrated)
        .await
        .map_err(WorkflowError::from)?;
    let changed = stored_ops.len();
    let ops_ps = changed as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
    tracing::debug!(?changed, %ops_ps, "ops integrated");
    let dna_hash = network.dna_hash().clone();

    // Record integrated ops metric.
    let dna_hash_str = dna_hash.to_string();
    workflow_integrated_op_metric().add(
        changed as u64,
        &[opentelemetry::KeyValue::new(
            "dna_hash",
            dna_hash_str.clone(),
        )],
    );

    // Record op integration delay metric.
    let metric = op_integration_delay_metric();
    when_stored_times
        .into_iter()
        // discard None values
        .flatten()
        .for_each(|when_stored| {
            let delay_secs =
                (when_integrated.as_micros() - when_stored.as_micros()).max(0) as f64 / 1_000_000.0;
            metric.record(
                delay_secs,
                &[opentelemetry::KeyValue::new(
                    "dna_hash",
                    dna_hash_str.clone(),
                )],
            );
        });

    // Record op validation attempts metric.
    let metric = op_validation_attempts_metric();
    validation_attempts
        .into_iter()
        // discard None values
        .flatten()
        .for_each(|attempts| {
            metric.record(
                attempts as u64,
                &[opentelemetry::KeyValue::new(
                    "dna_hash",
                    dna_hash_str.clone(),
                )],
            );
        });

    if changed > 0 {
        network.new_integrated_data(stored_ops).await?;

        update_local_authored_status(
            authored_db_provider.clone(),
            publish_trigger_provider,
            &dna_hash,
            when_integrated,
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
    authored_db_provider: Arc<dyn super::provider::authored_db_provider::AuthoredDbProvider>,
    publish_trigger_provider: Arc<
        dyn super::provider::publish_trigger_provider::PublishTriggerProvider,
    >,
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
            .await
            .map_err(WorkflowError::from)?
        else {
            continue;
        };

        db.write_async({
            let op_hashes = op_hashes.clone();
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
            "Marked authored ops as integrated"
        );

        let cell_id = CellId::new(dna_hash.clone(), author.clone());
        if let Some(trigger) = publish_trigger_provider.get_publish_trigger(&cell_id).await {
            tracing::debug!(?cell_id, "Triggering publish for integrated authored ops");
            trigger.trigger(&"integrate_dht_ops_workflow: authored ops marked as integrated");
        } else {
            tracing::error!(?cell_id, "No publish trigger for this cell");
        }
    }

    Ok(())
}
