//! The workflow and queue consumer for validation receipt

use super::*;
use crate::core::workflow::validation_receipt_workflow::validation_receipt_workflow;
use tracing::*;

/// Spawn the QueueConsumer for validation receipt workflow
#[instrument(skip(env, conductor, network))]
pub fn spawn_validation_receipt_consumer(
    dna_hash: Arc<DnaHash>,
    env: DbWrite<DbKindDht>,
    conductor: ConductorHandle,
    network: HolochainP2pDna,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();
    let keystore = conductor.keystore().clone();

    super::queue_consumer_dna_bound(
        "validation_receipt_consumer",
        dna_hash.clone(),
        conductor.task_manager(),
        (tx.clone(), rx),
        move || {
            validation_receipt_workflow(
                dna_hash.clone(),
                env.clone(),
                network.clone(),
                keystore.clone(),
                conductor.clone(),
            )
        },
    );

    tx
}
