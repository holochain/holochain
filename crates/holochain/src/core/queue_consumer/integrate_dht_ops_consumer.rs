//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::workflow::integrate_dht_ops_workflow::integrate_dht_ops_workflow;
use crate::{conductor::manager::ManagedTaskResult, core::workflow::error::WorkflowResult};
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for DhtOpIntegration workflow
#[instrument(skip(env, stop, trigger_sys, trigger_receipt))]
pub fn spawn_integrate_dht_ops_consumer(
    env: EnvWrite,
    mut stop: sync::broadcast::Receiver<()>,
    trigger_sys: sync::oneshot::Receiver<TriggerSender>,
    trigger_receipt: TriggerSender,
) -> (TriggerSender, JoinHandle<ManagedTaskResult>) {
    let (tx, mut rx) = TriggerSender::new();
    let trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        let trigger_sys = trigger_sys.await.expect("failed to get tx sys");
        loop {
            // Wait for next job
            if let Job::Shutdown = next_job_or_exit(&mut rx, &mut stop).await {
                tracing::warn!(
                    "Cell is shutting down: stopping integrate_dht_ops_workflow queue consumer."
                );
                break;
            }

            holochain_sqlite::db::optimistic_retry_async("integrate_dht_ops_consumer", || async {
                // Run the workflow
                if let WorkComplete::Incomplete = integrate_dht_ops_workflow(
                    env.clone(),
                    trigger_sys.clone(),
                    trigger_receipt.clone(),
                )
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
