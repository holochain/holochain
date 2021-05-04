//! The workflow and queue consumer for sys validation

use super::*;

use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
use crate::{conductor::manager::ManagedTaskResult, core::workflow::error::WorkflowResult};
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for Publish workflow
#[instrument(skip(env, stop, cell_network))]
pub fn spawn_publish_dht_ops_consumer(
    env: EnvWrite,
    mut stop: sync::broadcast::Receiver<()>,
    cell_network: HolochainP2pCell,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping publish_dht_ops_workflow queue consumer."
                );
                break;
            }

            holochain_sqlite::db::optimistic_retry_async("publish_dht_ops_consumer", || async {
                // Run the workflow
                if let WorkComplete::Incomplete =
                    publish_dht_ops_workflow(env.clone(), cell_network.clone()).await?
                {
                    trigger_self.clone().trigger()
                };
                WorkflowResult::Ok(())
            })
            .await
            .expect("Too many consecutive errors. Shutting down loop. TODO: make Holochain crash");
        }
        Ok(())
    });
    (tx, handle)
}
