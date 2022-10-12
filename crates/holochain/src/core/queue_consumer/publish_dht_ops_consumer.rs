//! The workflow and queue consumer for sys validation

use super::*;

use crate::conductor::manager::ManagedTaskFut;
use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
use tracing::*;

/// Spawn the QueueConsumer for Publish workflow
#[instrument(skip(env, conductor_handle, network))]
pub fn spawn_publish_dht_ops_consumer(
    agent: AgentPubKey,
    env: DbWrite<DbKindAuthored>,
    conductor_handle: ConductorHandle,
    network: Box<dyn HolochainP2pDnaT + Send + Sync>,
) -> (TriggerSender, impl ManagedTaskFut) {
    // Create a trigger with an exponential back off starting at 1 minute
    // and maxing out at 5 minutes.
    // The back off is reset any time the trigger is called (when new data is committed)

    let (tx, mut rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60 * 5), true);
    let trigger_self = tx.clone();
    let mut stop = conductor_handle.task_stopper().subscribe();
    let task = async move {
        let network = network;
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping publish_dht_ops_workflow queue consumer."
                );
                break;
            }

            if conductor_handle
                .config()
                .network
                .as_ref()
                .map(|c| c.tuning_params.disable_publish)
                .unwrap_or(false)
            {
                continue;
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
                    trigger_self.trigger(&"retrigger")
                }
                Err(err) => handle_workflow_error(err)?,
                _ => (),
            };
        }
        Ok(())
    };
    (tx, task)
}
