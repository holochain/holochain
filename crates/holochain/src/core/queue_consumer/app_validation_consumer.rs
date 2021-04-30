//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::workflow::app_validation_workflow::app_validation_workflow;
use crate::core::workflow::app_validation_workflow::AppValidationWorkspace;
use crate::{conductor::manager::ManagedTaskResult, core::workflow::error::WorkflowResult};
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for AppValidation workflow
#[instrument(skip(env, cache, stop, trigger_integration, conductor_api, network))]
pub fn spawn_app_validation_consumer(
    env: EnvWrite,
    cache: EnvWrite,
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

            holochain_sqlite::db::optimistic_retry_async("app_validation_consumer", || async {
                // Run the workflow
                let workspace = AppValidationWorkspace::new(env.clone().into(), cache.clone());
                if let WorkComplete::Incomplete = app_validation_workflow(
                    workspace,
                    trigger_integration.clone(),
                    conductor_api.clone(),
                    network.clone(),
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
