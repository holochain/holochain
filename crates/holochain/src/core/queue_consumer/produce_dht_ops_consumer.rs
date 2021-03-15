//! The workflow and queue consumer for DhtOp production

use super::*;
use crate::core::workflow::produce_dht_ops_workflow::produce_dht_ops_workflow;
use crate::core::workflow::produce_dht_ops_workflow::ProduceDhtOpsWorkspace;
use crate::{conductor::manager::ManagedTaskResult, core::workflow::error::WorkflowResult};
use holochain_sqlite::db::DbWrite;

use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for Produce_dht_ops workflow
#[instrument(skip(env, stop, trigger_publish))]
pub fn spawn_produce_dht_ops_consumer(
    env: DbWrite,
    mut stop: sync::broadcast::Receiver<()>,
    mut trigger_publish: TriggerSender,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let mut trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping produce_dht_ops_workflow queue consumer."
                );
                break;
            }
            if let Err(err) =
                produce_dht_ops_consumer_loop(env.clone(), &mut trigger_self, &mut trigger_publish)
                    .await
            {
                tracing::error!(
                    "Error in produce_dht_ops_consumer_loop, restarting loop: {:?}",
                    err
                )
            }
        }
        Ok(())
    });
    (tx, handle)
}

async fn produce_dht_ops_consumer_loop(
    env: DbWrite,
    trigger_self: &mut TriggerSender,
    trigger_publish: &mut TriggerSender,
) -> WorkflowResult<()> {
    let workspace = ProduceDhtOpsWorkspace::new(env.clone().into())?;
    if let WorkComplete::Incomplete =
        produce_dht_ops_workflow(workspace, env.clone().into(), trigger_publish).await?
    {
        trigger_self.trigger()
    };
    Ok(())
}
