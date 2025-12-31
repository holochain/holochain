//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::conductor::manager::TaskManagerClient;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_dht_ops_workflow;
use holochain_sqlite::error::DatabaseResult;
use must_future::MustBoxFuture;
use std::sync::Arc;

// Helper function to convert ConductorHandle to AuthoredDbProvider
fn as_authored_db_provider(
    conductor: crate::conductor::ConductorHandle,
) -> Arc<dyn crate::core::workflow::authored_db_provider::AuthoredDbProvider> {
    Arc::new(ConductorAsProvider(conductor))
        as Arc<dyn crate::core::workflow::authored_db_provider::AuthoredDbProvider>
}

// Helper function to convert ConductorHandle to PublishTriggerProvider
fn as_publish_trigger_provider(
    conductor: crate::conductor::ConductorHandle,
) -> Arc<dyn crate::core::workflow::publish_trigger_provider::PublishTriggerProvider> {
    Arc::new(ConductorAsProvider(conductor))
        as Arc<dyn crate::core::workflow::publish_trigger_provider::PublishTriggerProvider>
}

// Wrapper type to enable trait object conversion
struct ConductorAsProvider(crate::conductor::ConductorHandle);

impl crate::core::workflow::authored_db_provider::AuthoredDbProvider for ConductorAsProvider {
    fn get_authored_db(
        &self,
        dna_hash: &DnaHash,
        author: &AgentPubKey,
    ) -> MustBoxFuture<'_, DatabaseResult<Option<DbWrite<DbKindAuthored>>>> {
        self.0.get_authored_db(dna_hash, author)
    }
}

impl crate::core::workflow::publish_trigger_provider::PublishTriggerProvider
    for ConductorAsProvider
{
    fn get_publish_trigger(&self, cell_id: &CellId) -> MustBoxFuture<'_, Option<TriggerSender>> {
        self.0.get_publish_trigger(cell_id)
    }
}

/// Spawn the QueueConsumer for DhtOpIntegration workflow
#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(env, trigger_receipt, tm, network, conductor))
)]
pub fn spawn_integrate_dht_ops_consumer(
    dna_hash: Arc<DnaHash>,
    env: DbWrite<DbKindDht>,
    tm: TaskManagerClient,
    trigger_receipt: TriggerSender,
    network: DynHolochainP2pDna,
    conductor: crate::conductor::ConductorHandle,
) -> TriggerSender {
    let (tx, rx) = TriggerSender::new();

    let authored_db_provider = as_authored_db_provider(conductor.clone());
    let publish_trigger_provider = as_publish_trigger_provider(conductor.clone());

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
