//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_dht_ops_workflow;
use crate::core::workflow::integrate_dht_ops_workflow::IntegrateDhtOpsWorkspace;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for DhtOpIntegration workflow
#[instrument(skip(env, conductor_handle, stop, trigger_sys, trigger_receipt))]
pub fn spawn_integrate_dht_ops_consumer(
    env: EnvWrite,
    conductor_handle: ConductorHandle,
    cell_id: CellId,
    mut stop: sync::broadcast::Receiver<()>,
    trigger_sys: sync::oneshot::Receiver<TriggerSender>,
    trigger_receipt: TriggerSender,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let mut trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        let trigger_sys = trigger_sys.await.expect("failed to get tx sys");
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping integrate_dht_ops_workflow queue consumer."
                );
                break;
            }

            // Run the workflow
            let workspace = IntegrateDhtOpsWorkspace::new(env.clone().into())
                .expect("Could not create Workspace");
            match integrate_dht_ops_workflow(
                workspace,
                env.clone().into(),
                trigger_sys.clone(),
                trigger_receipt.clone(),
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
