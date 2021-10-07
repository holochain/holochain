//! The workflow and queue consumer for sys validation

use super::*;

use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for Publish workflow
#[instrument(skip(env, conductor_handle, stop, cell_network))]
pub fn spawn_publish_dht_ops_consumer(
    env: EnvWrite,
    conductor_handle: ConductorHandle,
    mut stop: sync::broadcast::Receiver<()>,
    cell_network: Box<dyn HolochainP2pCellT + Send + Sync>,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    // Create a trigger with an exponential back off starting at 1 minute
    // and maxing out at 5 minutes.
    // The back off is reset any time the trigger is called (when new data is committed)
    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60 * 5), true);
    let trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        let cell_network = cell_network;
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping publish_dht_ops_workflow queue consumer."
                );
                break;
            }

            #[cfg(any(test, feature = "test_utils"))]
            {
                if conductor_handle.should_skip_publish() {
                    continue;
                }
            }

            // Run the workflow
            match publish_dht_ops_workflow(env.clone(), cell_network.as_ref(), &trigger_self).await
            {
                Ok(WorkComplete::Incomplete) => trigger_self.trigger(),
                Err(err) => {
                    let cell_id = cell_network.cell_id();
                    handle_workflow_error(
                        conductor_handle.clone(),
                        cell_id,
                        err,
                        "publish_dht_ops failure",
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
