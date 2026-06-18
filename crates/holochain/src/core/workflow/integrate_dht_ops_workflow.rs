//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::metrics::{
    op_integration_delay_metric, op_validation_attempts_metric, workflow_integrated_op_metric,
};
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use holo_hash::{AgentPubKey, DhtOpHash, DnaHash};
use holochain_p2p::DynHolochainP2pDna;
use holochain_sqlite::db::DbKindDht;
use holochain_sqlite::prelude::DbWrite;
use holochain_state::dht_store::DhtStore;
use holochain_state::prelude::*;
use holochain_zome_types::dht_v2::OpValidity;
use kitsune2_api::StoredOp;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(vault, dht_store, trigger_receipt, network, authored_db_provider))
)]
pub async fn integrate_dht_ops_workflow(
    vault: DbWrite<DbKindDht>,
    dht_store: DhtStore,
    trigger_receipt: TriggerSender,
    network: DynHolochainP2pDna,
    authored_db_provider: Arc<dyn super::provider::authored_db_provider::AuthoredDbProvider>,
) -> WorkflowResult<WorkComplete> {
    let start = std::time::Instant::now();
    let when_integrated = Timestamp::now();

    let summaries = dht_store.integrate_ready_ops(when_integrated).await?;

    // Dual-write: mark all awaiting-integration ops in the legacy DhtOp table
    // so legacy readers (tests, integration dumps) see consistent state during
    // the read-migration window. This mirrors the former
    // SET_VALIDATED_OPS_TO_INTEGRATED SQL and covers both network-received ops
    // (which flow through the new DhtStore's limbo pathway) and locally-authored
    // ops (genesis, call_zome) which are inserted into the legacy DB as
    // validation_stage=3 but directly into the new DB as already-integrated.
    // This will be removed once the legacy DhtOp table is retired.
    let legacy_marked_pairs: Vec<(DhtOpHash, Option<AgentPubKey>)> = vault
        .write_async(
            move |txn| -> StateMutationResult<Vec<(DhtOpHash, Option<AgentPubKey>)>> {
                holochain_state::mutations::set_all_awaiting_integration_to_integrated(
                    txn,
                    when_integrated,
                )
            },
        )
        .await?;

    let changed = summaries.len();
    let legacy_marked = legacy_marked_pairs.len();
    let ops_ps = changed as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
    tracing::debug!(?changed, %ops_ps, "ops integrated");
    let dna_hash = network.dna_hash().clone();
    let dna_hash_str = dna_hash.to_string();

    let mut stored_ops: Vec<StoredOp> = Vec::with_capacity(summaries.len());
    let mut block_agents: Vec<(AgentPubKey, DhtOpHash)> = Vec::new();
    // Pairs from the new-DB summaries (network-received ops that went through limbo).
    let mut new_db_pairs: Vec<(DhtOpHash, Option<AgentPubKey>)> =
        Vec::with_capacity(summaries.len());

    for s in &summaries {
        stored_ops.push(StoredOp {
            created_at: kitsune2_api::Timestamp::from_micros(s.authored_timestamp.as_micros()),
            op_id: s.op_hash.to_located_k2_op_id(&s.basis_hash),
        });
        new_db_pairs.push((s.op_hash.clone(), s.action_author.clone()));

        if let (Some(warrantee), Some(warrant_author)) = (&s.warrantee, &s.warrant_author) {
            match s.validation_status {
                OpValidity::Accepted => {
                    tracing::info!(
                        ?warrantee,
                        op_hash = ?s.op_hash,
                        "Warrant op is valid, will block the warrantee"
                    );
                    block_agents.push((warrantee.clone(), s.op_hash.clone()));
                }
                OpValidity::Rejected => {
                    tracing::info!(
                        ?warrant_author,
                        op_hash = ?s.op_hash,
                        "Warrant op is invalid, will block the author"
                    );
                    block_agents.push((warrant_author.clone(), s.op_hash.clone()));
                }
            }
        }
    }

    // Record integrated ops metric.
    workflow_integrated_op_metric().add(
        changed as u64,
        &[opentelemetry::KeyValue::new(
            "dna_hash",
            dna_hash_str.clone(),
        )],
    );

    // Record op integration delay + validation attempts metrics. The
    // summaries path covers limbo-promoted ops; locally-authored ops bypass
    // limbo and only appear in `legacy_marked_pairs` — emit zeroed records
    // for them so the metrics fire on every integration tick that integrated
    // anything, matching develop's per-op emission.
    let summary_hashes: std::collections::HashSet<DhtOpHash> =
        summaries.iter().map(|s| s.op_hash.clone()).collect();
    let delay_metric = op_integration_delay_metric();
    let attempts_metric = op_validation_attempts_metric();
    for s in &summaries {
        let delay_secs =
            (when_integrated.as_micros() - s.when_received.as_micros()).max(0) as f64 / 1_000_000.0;
        delay_metric.record(
            delay_secs,
            &[opentelemetry::KeyValue::new(
                "dna_hash",
                dna_hash_str.clone(),
            )],
        );
        attempts_metric.record(
            s.validation_attempts as u64,
            &[opentelemetry::KeyValue::new(
                "dna_hash",
                dna_hash_str.clone(),
            )],
        );
    }
    for (op_hash, _) in &legacy_marked_pairs {
        if summary_hashes.contains(op_hash) {
            continue;
        }
        delay_metric.record(
            0.0,
            &[opentelemetry::KeyValue::new(
                "dna_hash",
                dna_hash_str.clone(),
            )],
        );
        attempts_metric.record(
            0,
            &[opentelemetry::KeyValue::new(
                "dna_hash",
                dna_hash_str.clone(),
            )],
        );
    }

    if changed > 0 || legacy_marked > 0 {
        // Combine new-DB pairs with legacy-only pairs (e.g. locally-authored ops
        // that bypassed LimboChainOp). Deduplicate in case an op appears in both.
        let mut seen = std::collections::HashSet::new();
        let all_integrated_pairs: Vec<(DhtOpHash, Option<AgentPubKey>)> = new_db_pairs
            .into_iter()
            .chain(legacy_marked_pairs)
            .filter(|(h, _)| seen.insert(h.clone()))
            .collect();

        update_local_authored_status(
            authored_db_provider.clone(),
            &dna_hash,
            when_integrated,
            all_integrated_pairs,
        )
        .await?;

        if changed > 0 {
            network.new_integrated_data(stored_ops).await?;

            // Block agents warranted for invalid ops.
            match InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()) {
                Ok(interval) => {
                    for (block_agent, invalid_op_hash) in block_agents {
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
        }

        Ok(WorkComplete::Incomplete(None))
    } else {
        Ok(WorkComplete::Complete)
    }
}

async fn update_local_authored_status(
    authored_db_provider: Arc<dyn super::provider::authored_db_provider::AuthoredDbProvider>,
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
            .await?
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
    }

    Ok(())
}
