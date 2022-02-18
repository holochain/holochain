//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::conductor::space::DhtDbQueryCache;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_dht_ops_workflow;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for DhtOpIntegration workflow
#[instrument(skip(env, stop, trigger_receipt, network, dht_query_cache))]
pub fn spawn_integrate_dht_ops_consumer(
    dna_hash: Arc<DnaHash>,
    env: DbWrite<DbKindDht>,
    dht_query_cache: DhtDbQueryCache,
    mut stop: sync::broadcast::Receiver<()>,
    trigger_receipt: TriggerSender,
    network: HolochainP2pDna,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping integrate_dht_ops_workflow queue consumer."
                );
                break;
            }

            // Run the workflow
            match integrate_dht_ops_workflow(
                env.clone(),
                &dht_query_cache,
                trigger_receipt.clone(),
                network.clone(),
            )
            .await
            {
                Ok(WorkComplete::Incomplete) => trigger_self.trigger(),
                Err(err) => handle_workflow_error(err)?,
                _ => (),
            };
        }
        Ok(())
    });
    (tx, handle)
}
