//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_dht_ops_workflow;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for DhtOpIntegration workflow
#[instrument(skip(env, conductor_handle, stop, trigger_receipt, cell_network))]
pub fn spawn_integrate_dht_ops_consumer(
    env: EnvWrite,
    conductor_handle: ConductorHandle,
    cell_id: CellId,
    mut stop: sync::broadcast::Receiver<()>,
    trigger_receipt: TriggerSender,
    cell_network: HolochainP2pCell,
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
                trigger_receipt.clone(),
                cell_network.clone(),
            )
            .await
            {
                Ok(WorkComplete::Incomplete) => trigger_self.trigger(),
                Err(err) => {
                    handle_workflow_error(
                        conductor_handle.clone(),
                        cell_id.clone(),
                        err,
                        "integrate_dht_ops failure",
                    )
                    .await?
                }
                _ => (),
            };
        }
        Ok(())
    });
    (tx, handle)
}
