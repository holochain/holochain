//! The workflow and queue consumer for DhtOp integration

use super::*;

use crate::conductor::manager::ManagedTaskResult;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_dht_ops_workflow;
use crate::core::workflow::integrate_dht_ops_workflow::IntegrateDhtOpsWorkspace;
use holochain_lmdb::env::EnvironmentWrite;

use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for DhtOpIntegration workflow
#[instrument(skip(env, stop, trigger_sys, trigger_receipt))]
pub fn spawn_integrate_dht_ops_consumer(
    env: EnvironmentWrite,
    mut stop: sync::broadcast::Receiver<()>,
    trigger_sys: sync::oneshot::Receiver<TriggerSender>,
    mut trigger_receipt: TriggerSender,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let mut trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        let mut trigger_sys = trigger_sys.await.expect("failed to get tx sys");
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping integrate_dht_ops_workflow queue consumer."
                );
                break;
            }

            // Run the workflow
            let workspace = IntegrateDhtOpsWorkspace::new(env.clone().into())
                .expect("Could not create Workspace");
            if let WorkComplete::Incomplete = integrate_dht_ops_workflow(
                workspace,
                env.clone().into(),
                &mut trigger_sys,
                &mut trigger_receipt,
            )
            .await
            .expect("Error running Workflow")
            {
                trigger_self.trigger()
            };
        }
        Ok(())
    });
    (tx, handle)
}
