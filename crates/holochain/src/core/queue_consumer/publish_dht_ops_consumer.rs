//! The workflow and queue consumer for sys validation

use super::*;

use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
use tracing::*;

/// Spawn the QueueConsumer for Publish workflow
#[instrument(skip(env, conductor, network))]
pub fn spawn_publish_dht_ops_consumer(
    cell_id: CellId,
    env: DbWrite<DbKindAuthored>,
    conductor: ConductorHandle,
    network: Arc<dyn HolochainP2pDnaT + Send + Sync>,
) -> TriggerSender {
    // Create a trigger with an exponential back off starting at 1 minute
    // and maxing out at 5 minutes.
    // The back off is reset any time the trigger is called (when new data is committed)
    let (tx, rx) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(60 * 5), true);
    let sender = tx.clone();
    super::queue_consumer_cell_bound(
        "publish_dht_ops_consumer",
        cell_id.clone(),
        conductor.task_manager(),
        (tx.clone(), rx),
        move || {
            let tx = tx.clone();
            let conductor = conductor.clone();
            let env = env.clone();
            let agent = cell_id.agent_pubkey().clone();
            let network = network.clone();
            let wf = publish_dht_ops_workflow(env, network, tx, agent);
            async move {
                if conductor
                    .get_config()
                    .network
                    .as_ref()
                    .map(|c| c.tuning_params.disable_publish)
                    .unwrap_or(false)
                {
                    Ok(WorkComplete::Complete)
                } else {
                    wf.await
                }
            }
        },
    );
    sender
}
