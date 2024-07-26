//! The workflow and queue consumer for validation receipt

use super::*;
use crate::core::workflow::validation_receipt_workflow::validation_receipt_workflow;
use futures::FutureExt;

/// Spawn the QueueConsumer for validation receipt workflow
#[cfg_attr(feature = "instrument", tracing::instrument(skip(env, conductor, network)))]
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
                conductor.running_cell_ids(),
                {
                    let conductor = conductor.clone();
                    move |block| {
                        let conductor = conductor.clone();
                        // This can be cleaned up when the compiler is smarter - https://github.com/rust-lang/rust/issues/69663
                        async move { conductor.block(block).await }.boxed()
                    }
                },
            )
        },
    );

    tx
}
