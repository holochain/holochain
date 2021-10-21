//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::countersigning_workflow::{
    countersigning_workflow, CountersigningWorkspace,
};
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for countersigning workflow
#[instrument(skip(dht_env, stop, workspace, cell_network, trigger_sys))]
pub(crate) fn spawn_countersigning_consumer(
    dht_env: DbWrite<DbKindDht>,
    mut stop: sync::broadcast::Receiver<()>,
    workspace: CountersigningWorkspace,
    cell_network: HolochainP2pDna,
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
            match countersigning_workflow(&dht_env, &workspace, &cell_network, &trigger_sys).await {
                Ok(WorkComplete::Incomplete) => trigger_self.trigger(),
                Err(err) => handle_workflow_error(err)?,
                _ => (),
            };
        }
        Ok(())
    });
    (tx, handle)
}
