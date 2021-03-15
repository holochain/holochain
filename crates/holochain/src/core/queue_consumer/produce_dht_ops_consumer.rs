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
    trigger_publish: TriggerSender,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping produce_dht_ops_workflow queue consumer."
                );
                break;
            }

            holochain_sqlite::db::optimistic_retry_async("produce_dht_ops_consumer", || async {
                let workspace = ProduceDhtOpsWorkspace::new(env.clone().into())?;
                if let WorkComplete::Incomplete =
                    produce_dht_ops_workflow(workspace, env.clone().into(), trigger_publish.clone())
                        .await?
                {
                    trigger_self.clone().trigger()
                };
                WorkflowResult::Ok(())
            })
            .await
            .expect("Too many consecutive errors. Shutting down loop. TODO: make Holochain crash");
        }
        Ok(())
    });
    (tx, handle)
}
