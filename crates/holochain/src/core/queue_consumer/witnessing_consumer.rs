//! The workflow and queue consumer for witnessing countersigning sessions.

use super::*;
use crate::conductor::manager::TaskManagerClient;
use crate::core::workflow::witnessing_workflow::witnessing_workflow;
use tracing::*;

/// Spawn the QueueConsumer for the witnessing workflow
#[instrument(skip_all)]
pub(crate) fn spawn_witnessing_consumer(
    space: Space,
    tm: TaskManagerClient,
    dna_network: HolochainP2pDna,
    trigger_sys: TriggerSender,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();

    super::queue_consumer_dna_bound(
        "witnessing_consumer",
        space.dna_hash.clone(),
        tm,
        (tx.clone(), rx),
        move || witnessing_workflow(space.clone(), dna_network.clone(), trigger_sys.clone()),
    );

    tx
}
