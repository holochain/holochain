//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::workflow::sys_validation_workflow::sys_validation_workflow;
use crate::core::workflow::sys_validation_workflow::SysValidationWorkspace;
use tracing::*;

/// Spawn the QueueConsumer for SysValidation workflow
#[instrument(skip(workspace, space, conductor, trigger_app_validation, network,))]
pub fn spawn_sys_validation_consumer(
    workspace: SysValidationWorkspace,
    space: Space,
    conductor: ConductorHandle,
    trigger_app_validation: TriggerSender,
    network: HolochainP2pDna,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let workspace = Arc::new(workspace);
    let space = Arc::new(space);

    super::queue_consumer_dna_bound(
        "sys_validation_consumer",
        space.dna_hash.clone(),
        conductor.task_manager(),
        (tx.clone(), rx),
        move || {
            sys_validation_workflow(
                workspace.clone(),
                space.clone(),
                trigger_app_validation.clone(),
                trigger_self.clone(),
                network.clone(),
                conductor.clone(),
            )
        },
    );

    tx
}
