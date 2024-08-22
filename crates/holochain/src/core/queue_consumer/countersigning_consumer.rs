//! The queue consumer for the countersigning workflow.

use super::*;
use crate::conductor::manager::TaskManagerClient;
use crate::core::workflow::countersigning_workflow::countersigning_workflow;
use tracing::*;

/// Spawn the QueueConsumer for the countersigning workflow
#[instrument(skip_all)]
pub(crate) fn spawn_countersigning_consumer(
    space: Space,
    task_manager: TaskManagerClient,
    dna_network: HolochainP2pDna,
    cell_id: CellId,
    conductor: ConductorHandle,
    trigger_sys: TriggerSender,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();

    let self_trigger = tx.clone();
    queue_consumer_dna_bound(
        "countersigning_consumer",
        space.dna_hash.clone(),
        task_manager,
        (tx.clone(), rx),
        move || {
            countersigning_workflow(
                space.clone(),
                dna_network.clone(),
                cell_id.clone(),
                conductor.clone(),
                self_trigger.clone(),
                trigger_sys.clone(),
                integration_trigger.clone(),
                publish_trigger.clone(),
            )
        },
    );

    tx
}
