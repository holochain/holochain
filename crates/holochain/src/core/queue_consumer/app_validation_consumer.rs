//! The workflow and queue consumer for sys validation

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::app_validation_workflow::app_validation_workflow;
use crate::core::workflow::app_validation_workflow::AppValidationWorkspace;
use holochain_state::env::EnvironmentWrite;

use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for AppValidation workflow
#[instrument(skip(env, stop, trigger_integration, conductor_api, network))]
pub fn spawn_app_validation_consumer(
    env: EnvironmentWrite,
    mut stop: sync::broadcast::Receiver<()>,
    mut trigger_integration: TriggerSender,
    conductor_api: impl CellConductorApiT + 'static,
    network: HolochainP2pCell,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let mut trigger_self = tx.clone();
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
            let workspace = AppValidationWorkspace::new(env.clone().into())
                .expect("Could not create Workspace");
            if let WorkComplete::Incomplete = app_validation_workflow(
                workspace,
                env.clone().into(),
                &mut trigger_integration,
                conductor_api.clone(),
                network.clone(),
            )
            .await
            .expect("Error running Workflow")
            {
                trigger_self.trigger()
            };
        }
        Ok(())
    });
    (tx, handle)
}
