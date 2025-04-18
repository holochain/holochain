//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::TaskManagerClient;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_dht_ops_workflow;

/// Spawn the QueueConsumer for DhtOpIntegration workflow
#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(env, trigger_receipt, tm, network))
)]
pub fn spawn_integrate_dht_ops_consumer(
    dna_hash: Arc<DnaHash>,
    env: DbWrite<DbKindDht>,
    tm: TaskManagerClient,
    trigger_receipt: TriggerSender,
    network: HolochainP2pDna,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();

    super::queue_consumer_dna_bound(
        "integrate_dht_ops_consumer",
        dna_hash,
        tm,
        (tx.clone(), rx),
        move || integrate_dht_ops_workflow(env.clone(), trigger_receipt.clone(), network.clone()),
    );

    tx
}
