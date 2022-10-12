//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::workflow::app_validation_workflow::app_validation_workflow;
use crate::core::workflow::app_validation_workflow::AppValidationWorkspace;
use holochain_p2p::*;
use holochain_types::db_cache::DhtDbQueryCache;
use tracing::*;

/// Spawn the QueueConsumer for AppValidation workflow
#[instrument(skip(
    workspace,
    conductor_handle,
    trigger_integration,
    network,
    dht_query_cache
))]
pub fn spawn_app_validation_consumer(
    dna_hash: Arc<DnaHash>,
    workspace: AppValidationWorkspace,
    conductor_handle: ConductorHandle,
    trigger_integration: TriggerSender,
    network: HolochainP2pDna,
    dht_query_cache: DhtDbQueryCache,
) -> (TriggerSender, impl ManagedTaskFut) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let workspace = Arc::new(workspace);
    let mut stop = conductor_handle.task_stopper().subscribe();
    let task = async move {
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping app_validation_workflow queue consumer."
                );
                break;
            }

            // Run the workflow
            let result = app_validation_workflow(
                dna_hash.clone(),
                workspace.clone(),
                trigger_integration.clone(),
                conductor_handle.clone(),
                network.clone(),
                dht_query_cache.clone(),
            )
            .await;
            match result {
                Ok(WorkComplete::Incomplete) => {
                    tracing::debug!("Work incomplete, retriggering workflow");
                    trigger_self.trigger(&"retrigger")
                }
                Err(err) => handle_workflow_error(err)?,
                _ => (),
            };
        }
        Ok(())
    };
    (tx, task)
}
