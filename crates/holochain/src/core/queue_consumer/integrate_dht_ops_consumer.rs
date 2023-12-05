//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::TaskManagerClient;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_dht_ops_workflow;
use holochain_types::db_cache::DhtDbQueryCache;

use tracing::*;

/// Spawn the QueueConsumer for DhtOpIntegration workflow
#[instrument(skip(env, trigger_receipt, tm, network, dht_query_cache))]
pub fn spawn_integrate_dht_ops_consumer(
    dna_hash: Arc<DnaHash>,
    env: DbWrite<DbKindDht>,
    dht_query_cache: DhtDbQueryCache,
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
        move || {
            integrate_dht_ops_workflow(
                env.clone(),
                dht_query_cache.clone(),
                trigger_receipt.clone(),
                network.clone(),
            )
        },
    );

    tx
}
