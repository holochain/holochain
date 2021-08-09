//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::countersigning_workflow::{
    countersigning_workflow, CountersigningWorkspace,
};
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for countersigning workflow
#[instrument(skip(env, stop, conductor_handle, workspace, cell_network, trigger_sys))]
pub(crate) fn spawn_countersigning_consumer(
    env: EnvWrite,
    mut stop: sync::broadcast::Receiver<()>,
    conductor_handle: ConductorHandle,
    workspace: CountersigningWorkspace,
    cell_network: HolochainP2pCell,
    mut trigger_sys: TriggerSender,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let mut trigger_self = tx.clone();
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
            match countersigning_workflow(&env, &workspace, &cell_network, &mut trigger_sys).await {
                Ok(WorkComplete::Incomplete) => trigger_self.trigger(),
                Err(err) => {
                    handle_workflow_error(
                        conductor_handle.clone(),
                        cell_network.cell_id(),
                        err,
                        "countersigning failure",
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
