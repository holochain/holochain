//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::countersigning_workflow::countersigning_workflow;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for countersigning workflow
#[instrument(skip(space, stop, dna_network, trigger_sys))]
pub(crate) fn spawn_countersigning_consumer(
    space: Space,
    mut stop: sync::broadcast::Receiver<()>,
    dna_network: HolochainP2pDna,
    trigger_sys: TriggerSender,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping countersigning_workflow queue consumer."
                );
                break;
            }

            // Run the workflow
            match countersigning_workflow(&space, &dna_network, &trigger_sys).await {
                Ok(WorkComplete::Incomplete) => {
                    tracing::debug!("Work incomplete, retriggering workflow");
                    trigger_self.trigger(&"retrigger")
                }
                Err(err) => handle_workflow_error(err)?,
                _ => (),
            };
        }
        Ok(())
    });
    (tx, handle)
}
