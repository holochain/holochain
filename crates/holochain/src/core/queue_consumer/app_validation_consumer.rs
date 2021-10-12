//! The workflow and queue consumer for sys validation

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::app_validation_workflow::app_validation_workflow;
use crate::core::workflow::app_validation_workflow::AppValidationWorkspace;
use holochain_p2p::*;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for AppValidation workflow
#[instrument(skip(
    env,
    cache,
    conductor_handle,
    stop,
    trigger_integration,
    conductor_api,
    network
))]
pub fn spawn_app_validation_consumer(
    env: EnvWrite,
    cache: EnvWrite,
    conductor_handle: ConductorHandle,
    mut stop: sync::broadcast::Receiver<()>,
    trigger_integration: TriggerSender,
    conductor_api: impl CellConductorApiT + 'static,
    network: HolochainP2pCell,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping app_validation_workflow queue consumer."
                );
                break;
            }

            // Run the workflow
            let workspace = AppValidationWorkspace::new(env.clone(), cache.clone());
            let result = app_validation_workflow(
                workspace,
                trigger_integration.clone(),
                conductor_api.clone(),
                network.clone(),
            )
            .await;
            match result {
                Ok(WorkComplete::Incomplete) => trigger_self.trigger(),
                Err(err) => {
                    handle_workflow_error(
                        conductor_handle.clone(),
                        network.cell_id(),
                        err,
                        "app_validation failure",
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
