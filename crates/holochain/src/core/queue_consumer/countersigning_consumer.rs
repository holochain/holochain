//! The queue consumer for the countersigning workflow.

use super::*;
use crate::core::workflow::countersigning_workflow::countersigning_workflow;
use tracing::*;

/// Spawn the QueueConsumer for the countersigning workflow
#[instrument(skip_all)]
pub(crate) fn spawn_countersigning_consumer(
    space: Space,
    dna_network: HolochainP2pDna,
    cell_id: CellId,
    conductor: ConductorHandle,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();

    let self_trigger = tx.clone();
    queue_consumer_dna_bound(
        "countersigning_consumer",
        space.dna_hash.clone(),
        conductor.task_manager(),
        (tx.clone(), rx),
        move || {
            countersigning_workflow(
                space.clone(),
                Arc::new(dna_network.clone()),
                cell_id.clone(),
                conductor.clone(),
                self_trigger.clone(),
                integration_trigger.clone(),
                publish_trigger.clone(),
            )
        },
    );

    tx
}
