//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::workflow::sys_validation_workflow::validation_deps::ValidationDependencies;
use crate::core::workflow::sys_validation_workflow::SysValidationWorkspace;
use crate::core::workflow::sys_validation_workflow::{
    get_representative_agent, sys_validation_workflow,
};
use holochain_keystore::MetaLairClient;
use parking_lot::Mutex;
use tracing::*;

/// Spawn the QueueConsumer for SysValidation workflow
#[instrument(skip(workspace, space, conductor, trigger_app_validation, network, keystore))]
pub fn spawn_sys_validation_consumer(
    workspace: SysValidationWorkspace,
    space: Space,
    conductor: ConductorHandle,
    trigger_app_validation: TriggerSender,
    network: HolochainP2pDna,
    keystore: MetaLairClient,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let workspace = Arc::new(workspace);
    let space = Arc::new(space);
    let config = conductor.config.clone();

    let current_validation_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));

    super::queue_consumer_dna_bound(
        "sys_validation_consumer",
        space.dna_hash.clone(),
        conductor.task_manager(),
        (tx.clone(), rx),
        move || {
            if let Some(representative_agent) =
                get_representative_agent(&conductor, &network.dna_hash())
            {
                Either::Left(sys_validation_workflow(
                    workspace.clone(),
                    current_validation_dependencies.clone(),
                    trigger_app_validation.clone(),
                    trigger_self.clone(),
                    network.clone(),
                    config.clone(),
                    keystore.clone(),
                    representative_agent,
                ))
            } else {
                tracing::warn!("No agent found for DNA, skipping sys validation");
                Either::Right(async move { Ok(WorkComplete::Complete) })
            }
        },
    );

    tx
}
