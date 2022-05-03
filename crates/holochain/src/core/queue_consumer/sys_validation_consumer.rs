//! The workflow and queue consumer for sys validation

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::sys_validation_workflow::sys_validation_workflow;
use crate::core::workflow::sys_validation_workflow::SysValidationWorkspace;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for SysValidation workflow
#[instrument(skip(
    workspace,
    space,
    conductor_handle,
    stop,
    trigger_app_validation,
    network,
))]
pub fn spawn_sys_validation_consumer(
    workspace: SysValidationWorkspace,
    space: Space,
    conductor_handle: ConductorHandle,
    mut stop: sync::broadcast::Receiver<()>,
    trigger_app_validation: TriggerSender,
    network: HolochainP2pDna,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let workspace = Arc::new(workspace);
    let space = Arc::new(space);
    let handle = tokio::spawn(async move {
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping sys_validation_workflow queue consumer."
                );
                break;
            }

            // Run the workflow
            match sys_validation_workflow(
                workspace.clone(),
                space.clone(),
                trigger_app_validation.clone(),
                trigger_self.clone(),
                network.clone(),
                conductor_handle.clone(),
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
