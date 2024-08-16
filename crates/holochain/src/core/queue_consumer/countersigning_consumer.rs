//! The queue consumer for the countersigning workflow.

use super::*;
use crate::conductor::manager::TaskManagerClient;
use crate::core::workflow::countersigning_workflow::countersigning_workflow;
use tracing::*;

/// Spawn the QueueConsumer for the witnessing workflow
#[instrument(skip_all)]
pub(crate) fn spawn_countersigning_consumer(
    space: Space,
    task_manager: TaskManagerClient,
    dna_network: HolochainP2pDna,
    trigger_sys: TriggerSender,
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
                self_trigger.clone(),
                trigger_sys.clone(),
            )
        },
    );

    tx
}
