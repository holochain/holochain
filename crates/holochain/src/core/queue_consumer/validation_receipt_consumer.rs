//! The workflow and queue consumer for validation receipt

use super::*;
use crate::conductor::manager::ManagedTaskFut;
use crate::core::workflow::validation_receipt_workflow::validation_receipt_workflow;
use tracing::*;

/// Spawn the QueueConsumer for validation receipt workflow
#[instrument(skip(env, conductor_handle, network))]
pub fn spawn_validation_receipt_consumer(
    dna_hash: Arc<DnaHash>,
    env: DbWrite<DbKindDht>,
    conductor_handle: ConductorHandle,
    network: HolochainP2pDna,
) -> (TriggerSender, impl ManagedTaskFut) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let keystore = conductor_handle.keystore().clone();
    let handle = async move {
        let mut stop = conductor_handle.task_stopper().subscribe();
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping validation_receipt_workflow queue consumer."
                );
                break;
            }

            // Run the workflow
            match validation_receipt_workflow(
                dna_hash.clone(),
                env.clone(),
                &network,
                keystore.clone(),
                conductor_handle.clone(),
            )
            .await
            {
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
    (tx, handle)
}
