//! The workflow and queue consumer for sys validation

use super::*;

use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
use crate::core::workflow::publish_dht_ops_workflow::PublishDhtOpsWorkspace;
use holochain_lmdb::env::EnvironmentWrite;

use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for Publish workflow
#[instrument(skip(env, conductor_handle, stop, cell_network))]
pub fn spawn_publish_dht_ops_consumer(
    env: EnvironmentWrite,
    conductor_handle: ConductorHandle,
    mut stop: sync::broadcast::Receiver<()>,
    mut cell_network: HolochainP2pCell,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let mut trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping publish_dht_ops_workflow queue consumer."
                );
                break;
            }

            // Run the workflow
            let workspace = PublishDhtOpsWorkspace::new(env.clone().into())
                .expect("Could not create Workspace");
            match publish_dht_ops_workflow(workspace, env.clone().into(), &mut cell_network).await {
                Ok(WorkComplete::Incomplete) => trigger_self.trigger(),
                Err(err) => {
                    handle_workflow_error(
                        conductor_handle.clone(),
                        cell_network.cell_id(),
                        err,
                        "publish_dht_ops failure",
                    )
                    .await?;
                    break;
                }
                _ => (),
            };
        }
        Ok(())
    });
    (tx, handle)
}
