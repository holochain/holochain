//! The workflow and queue consumer for sys validation

use super::*;
use crate::{
    conductor::manager::ManagedTaskResult,
    core::workflow::publish_dht_ops_workflow::{publish_dht_ops_workflow, PublishDhtOpsWorkspace},
};
use futures::future::Either;
use holochain_state::env::EnvironmentWrite;
use holochain_state::env::ReadManager;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;

// Minimum time between publishes to avoid spamming the network
const PUBLISH_RATE_MS: u64 = 500;

/// Spawn the QueueConsumer for Publish workflow
pub fn spawn_publish_dht_ops_consumer(
    env: EnvironmentWrite,
    mut stop: sync::broadcast::Receiver<()>,
    mut cell_network: HolochainP2pCell,
) -> (
    TriggerSender,
    tokio::sync::oneshot::Receiver<()>,
    JoinHandle<ManagedTaskResult>,
) {
    let (tx, mut rx) = TriggerSender::new();
    let (tx_first, rx_first) = tokio::sync::oneshot::channel();
    let mut tx_first = Some(tx_first);
    let mut trigger_self = tx.clone();
    let mut last_run_time = Instant::now();
    let handle = tokio::spawn(async move {
        loop {
            let env_ref = env.guard().await;
            let reader = env_ref.reader().expect("Could not create LMDB reader");
            let workspace =
                PublishDhtOpsWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");
            if let WorkComplete::Incomplete = publish_dht_ops_workflow(workspace, &mut cell_network)
                .await
                .expect("Error running Workflow")
            {
                trigger_self.trigger()
            };
            // notify the Cell that the first loop has completed
            if let Some(tx_first) = tx_first.take() {
                let _ = tx_first.send(());
            }

            last_run_time = Instant::now();

            // Check for shutdown or next job
            let next_job = rx.listen();
            let kill = stop.recv();
            tokio::pin!(next_job);
            tokio::pin!(kill);

            if let Either::Left((Err(_), _)) | Either::Right((_, _)) =
                futures::future::select(next_job, kill).await
            {
                tracing::warn!("Cell is shutting down: stopping queue consumer.");
                break;
            };

            // Delay for a bit if needed
            if let Some(t) =
                Duration::from_millis(PUBLISH_RATE_MS).checked_sub(last_run_time.elapsed())
            {
                tokio::time::delay_for(t).await;
            }
        }
        Ok(())
    });
    (tx, rx_first, handle)
}
