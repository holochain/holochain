//! The workflow and queue consumer for DhtOp production

use super::*;
use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::produce_dht_ops_workflow::produce_dht_ops_workflow;
use crate::core::workflow::produce_dht_ops_workflow::ProduceDhtOpsWorkspace;
use holochain_lmdb::env::EnvironmentWrite;

use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for Produce_dht_ops workflow
#[instrument(skip(env, conductor_handle, cell_id, stop, trigger_publish))]
pub fn spawn_produce_dht_ops_consumer(
    env: EnvironmentWrite,
    conductor_handle: ConductorHandle,
    cell_id: CellId,
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

            let workspace = ProduceDhtOpsWorkspace::new(env.clone().into())
                .expect("Could not create Workspace");
            match produce_dht_ops_workflow(workspace, env.clone().into(), &mut trigger_publish)
                .await
            {
                Ok(WorkComplete::Incomplete) => trigger_self.trigger(),
                Err(err) => {
                    handle_workflow_error(
                        conductor_handle.clone(),
                        cell_id.clone(),
                        err,
                        "produce_dht_ops failure",
                    )
                    .await?
                }
                _ => (),
            };
        }
        Ok(())
    });
    (tx, handle)
}
