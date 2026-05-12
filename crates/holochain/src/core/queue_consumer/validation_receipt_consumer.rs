//! The workflow and queue consumer for validation receipt

use super::*;
use crate::core::workflow::validation_receipt_workflow::validation_receipt_workflow;
use holochain_state::dht_store::DhtStore;

/// Spawn the QueueConsumer for validation receipt workflow
#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(env, dht_store, conductor, network))
)]
pub fn spawn_validation_receipt_consumer(
    dna_hash: Arc<DnaHash>,
    env: DbWrite<DbKindDht>,
    dht_store: DhtStore,
    conductor: ConductorHandle,
    network: DynHolochainP2pDna,
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
                dht_store.clone(),
                network.clone(),
                keystore.clone(),
                conductor.running_cell_ids(),
            )
        },
    );

    tx
}
