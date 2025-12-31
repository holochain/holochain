//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::TaskManagerClient;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_dht_ops_workflow;

/// Spawn the QueueConsumer for DhtOpIntegration workflow
#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(env, trigger_receipt, tm, network, authored_db_provider, publish_trigger_provider))
)]
pub fn spawn_integrate_dht_ops_consumer(
    dna_hash: Arc<DnaHash>,
    env: DbWrite<DbKindDht>,
    tm: TaskManagerClient,
    trigger_receipt: TriggerSender,
    network: DynHolochainP2pDna,
    authored_db_provider: Arc<crate::conductor::conductor::Conductor>,
    publish_trigger_provider: Arc<crate::conductor::conductor::Conductor>,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();

    super::queue_consumer_dna_bound(
        "integrate_dht_ops_consumer",
        dna_hash,
        tm,
        (tx.clone(), rx),
        move || {
            integrate_dht_ops_workflow(
                env.clone(),
                trigger_receipt.clone(),
                network.clone(),
                authored_db_provider.clone(),
                publish_trigger_provider.clone(),
            )
        },
    );

    tx
}
