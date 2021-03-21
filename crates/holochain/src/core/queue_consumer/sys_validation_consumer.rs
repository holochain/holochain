//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::workflow::sys_validation_workflow::sys_validation_workflow;
use crate::core::workflow::sys_validation_workflow::SysValidationWorkspace;
use crate::{conductor::manager::ManagedTaskResult, core::workflow::error::WorkflowResult};
use  tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for SysValidation workflow
#[instrument(skip(env, stop, trigger_app_validation, network, conductor_api))]
pub fn spawn_sys_validation_consumer(
    env: EnvWrite,
    mut stop: sync::broadcast::Receiver<()>,
    trigger_app_validation: TriggerSender,
    network: HolochainP2pCell,
    conductor_api: impl CellConductorApiT + 'static,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping sys_validation_workflow queue consumer."
                );
                break;
            }

            holochain_sqlite::db::optimistic_retry_async("sys_validation_consumer", || async {
                // Run the workflow
                let workspace = SysValidationWorkspace::new(env.clone().into())?;
                if let WorkComplete::Incomplete = sys_validation_workflow(
                    workspace,
                    env.clone().into(),
                    trigger_app_validation.clone(),
                    trigger_self.clone(),
                    network.clone(),
                    conductor_api.clone(),
                )
                .await?
                {
                    trigger_self.clone().trigger()
                };
                WorkflowResult::Ok(())
            })
            .await
            .expect("Too many consecutive errors. Shutting down loop. TODO: make Holochain crash");
        }
        Ok(())
    });
    (tx, handle)
}
