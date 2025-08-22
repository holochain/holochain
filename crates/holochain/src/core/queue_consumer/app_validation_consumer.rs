//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::workflow::app_validation_workflow::app_validation_workflow;
use crate::core::workflow::app_validation_workflow::AppValidationWorkspace;
use crate::core::workflow::sys_validation_workflow::get_representative_agent;

/// Spawn the QueueConsumer for AppValidation workflow
#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(
        workspace,
        conductor,
        trigger_integration,
        trigger_publish,
        network,
    ))
)]
pub fn spawn_app_validation_consumer(
    dna_hash: Arc<DnaHash>,
    workspace: AppValidationWorkspace,
    conductor: ConductorHandle,
    trigger_integration: TriggerSender,
    trigger_publish: TriggerSender,
    network: DynHolochainP2pDna,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();
    let workspace = Arc::new(workspace);

    queue_consumer_dna_bound(
        "app_validation_consumer",
        dna_hash.clone(),
        conductor.task_manager(),
        (tx.clone(), rx),
        move || {
            if let Some(representative_agent) =
                get_representative_agent(&conductor, &network.dna_hash())
            {
                Either::Left(app_validation_workflow(
                    dna_hash.clone(),
                    workspace.clone(),
                    trigger_integration.clone(),
                    trigger_publish.clone(),
                    conductor.clone(),
                    network.clone(),
                    representative_agent,
                ))
            } else {
                tracing::warn!("No representative agent found for DNA, skipping app validation.");
                Either::Right(async move { Ok(WorkComplete::Complete) })
            }
        },
    );
    tx
}
