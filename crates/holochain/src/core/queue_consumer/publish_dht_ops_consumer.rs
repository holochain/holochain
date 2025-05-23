//! The workflow and queue consumer for sys validation

use super::*;

use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;

/// Spawn the QueueConsumer for Publish workflow
#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(env, conductor, network))
)]
pub fn spawn_publish_dht_ops_consumer(
    cell_id: CellId,
    env: DbWrite<DbKindAuthored>,
    conductor: ConductorHandle,
    network: DynHolochainP2pDna,
) -> TriggerSender {
    #[cfg(feature = "test_utils")]
    let publish_override_interval = {
        let interval = conductor
            .get_config()
            .conductor_tuning_params()
            .publish_trigger_interval;

        interval.map(|i| i..i)
    };
    #[cfg(not(feature = "test_utils"))]
    let publish_override_interval = None;

    // Create a trigger with an exponential back off starting at 1 minute
    // and maxing out at 5 minutes.
    // The back off is reset any time the trigger is called (when new data is committed)
    let (tx, rx) = TriggerSender::new_with_loop(
        publish_override_interval
            .unwrap_or_else(|| Duration::from_secs(60)..Duration::from_secs(60 * 5)),
        true,
    );
    let sender = tx.clone();
    super::queue_consumer_cell_bound(
        "publish_dht_ops_consumer",
        cell_id.clone(),
        conductor.task_manager(),
        (tx.clone(), rx),
        move || {
            let conductor = conductor.clone();
            let tx = tx.clone();
            let env = env.clone();
            let agent = cell_id.agent_pubkey().clone();
            let network = network.clone();
            let min_publish_interval = conductor
                .get_config()
                .conductor_tuning_params()
                .min_publish_interval();
            async move {
                #[cfg(feature = "test_utils")]
                if conductor.get_config().network.disable_publish {
                    return Ok(WorkComplete::Complete);
                }

                publish_dht_ops_workflow(env, network, tx, agent, min_publish_interval).await
            }
        },
    );
    sender
}
