//! The workflow and queue consumer for sys validation

use super::*;

use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for Publish workflow
#[instrument(skip(env, conductor_handle, stop, network))]
pub fn spawn_publish_dht_ops_consumer(
    agent: AgentPubKey,
    env: DbWrite<DbKindAuthored>,
    conductor_handle: ConductorHandle,
    mut stop: sync::broadcast::Receiver<()>,
    network: Box<dyn HolochainP2pDnaT + Send + Sync>,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    // Create a trigger with an exponential back off starting at 1 minute
    // and maxing out at 5 minutes.
    // The back off is reset any time the trigger is called (when new data is committed)

    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60 * 5), true);
    let trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        let network = network;
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
                if !conductor_handle.dev_settings().publish {
                    continue;
                }
            }

            // Run the workflow
            match publish_dht_ops_workflow(
                env.clone(),
                network.as_ref(),
                &trigger_self,
                agent.clone(),
            )
            .await
            {
                Ok(WorkComplete::Incomplete) => {
                    tracing::debug!("Work incomplete, retriggering workflow");
                    trigger_self.trigger("retrigger")
                }
                Err(err) => handle_workflow_error(err)?,
                _ => (),
            };
        }
        Ok(())
    });
    (tx, handle)
}
