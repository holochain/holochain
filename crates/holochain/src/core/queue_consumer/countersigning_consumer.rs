//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::TaskManagerClient;
use crate::core::workflow::countersigning_workflow::countersigning_workflow;
use tracing::*;

/// Spawn the QueueConsumer for countersigning workflow
#[instrument(skip(space, tm, dna_network, trigger_sys))]
pub(crate) fn spawn_countersigning_consumer(
    space: Space,
    tm: TaskManagerClient,
    dna_network: HolochainP2pDna,
    trigger_sys: TriggerSender,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();

    super::queue_consumer_dna_bound(
        "countersigning_consumer",
        space.dna_hash.clone(),
        tm,
        (tx.clone(), rx),
        move || countersigning_workflow(space.clone(), dna_network.clone(), trigger_sys.clone()),
    );

    tx
}
