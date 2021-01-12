//! The workflow and queue consumer for sys validation

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::sys_validation_workflow::sys_validation_workflow;
use crate::core::workflow::sys_validation_workflow::SysValidationWorkspace;
use holochain_sqlite::env::EnvironmentWrite;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for SysValidation workflow
#[instrument(skip(env, stop, trigger_app_validation, network, conductor_api))]
pub fn spawn_sys_validation_consumer(
    env: EnvironmentWrite,
    mut stop: sync::broadcast::Receiver<()>,
    mut trigger_app_validation: TriggerSender,
    network: HolochainP2pCell,
    conductor_api: impl CellConductorApiT + 'static,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let mut trigger_self = tx.clone();
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
            let workspace = SysValidationWorkspace::new(env.clone().into())
                .expect("Could not create Workspace");
            if let WorkComplete::Incomplete = sys_validation_workflow(
                workspace,
                env.clone().into(),
                &mut trigger_app_validation,
                trigger_self.clone(),
                network.clone(),
                conductor_api.clone(),
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
