//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::metrics::{
    op_integration_delay_metric, op_validation_attempts_metric, workflow_integrated_op_metric,
};
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_p2p::DynHolochainP2pDna;
use holochain_state::dht_store::DhtStore;
use holochain_state::prelude::*;
use holochain_zome_types::dht_v2::OpValidity;
use kitsune2_api::StoredOp;

#[cfg(test)]
mod tests;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(dht_store, trigger_receipt, network))
)]
pub async fn integrate_dht_ops_workflow(
    dht_store: DhtStore,
    trigger_receipt: TriggerSender,
    network: DynHolochainP2pDna,
) -> WorkflowResult<WorkComplete> {
    let start = std::time::Instant::now();
    let when_integrated = Timestamp::now();

    let summaries = dht_store.integrate_ready_ops(when_integrated).await?;

    let changed = summaries.len();
    let ops_ps = changed as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
    tracing::debug!(?changed, %ops_ps, "ops integrated");
    let dna_hash = network.dna_hash().clone();
    let dna_hash_str = dna_hash.to_string();

    let mut stored_ops: Vec<StoredOp> = Vec::with_capacity(summaries.len());
    let mut block_agents: Vec<(AgentPubKey, DhtOpHash)> = Vec::new();

    for s in &summaries {
        stored_ops.push(StoredOp {
            created_at: kitsune2_api::Timestamp::from_micros(s.authored_timestamp.as_micros()),
            op_id: s.op_hash.to_located_k2_op_id(&s.basis_hash),
        });

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

    // Record op integration delay + validation attempts metrics.
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

        Ok(WorkComplete::Incomplete(None))
    } else {
        Ok(WorkComplete::Complete)
    }
}
