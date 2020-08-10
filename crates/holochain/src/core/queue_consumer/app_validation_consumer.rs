//! The workflow and queue consumer for sys validation

use super::*;
use crate::{
    conductor::manager::ManagedTaskResult,
    core::{
        state::workspace::Workspace,
        workflow::app_validation_workflow::{app_validation_workflow, AppValidationWorkspace},
    },
};
use futures::future::Either;
use holochain_state::env::EnvironmentWrite;
use holochain_state::env::ReadManager;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for AppValidation workflow
#[instrument(skip(env, stop, trigger_integration))]
pub fn spawn_app_validation_consumer(
    env: EnvironmentWrite,
    mut stop: sync::broadcast::Receiver<()>,
    mut trigger_integration: TriggerSender,
) -> (
    TriggerSender,
    tokio::sync::oneshot::Receiver<()>,
    JoinHandle<ManagedTaskResult>,
) {
    let (tx, mut rx) = TriggerSender::new();
    let (tx_first, rx_first) = tokio::sync::oneshot::channel();
    let mut tx_first = Some(tx_first);
    let mut trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            let env_ref = env.guard().await;
            let reader = env_ref.reader().expect("Could not create LMDB reader");
            let workspace =
                AppValidationWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");
            if let WorkComplete::Incomplete =
                app_validation_workflow(workspace, env.clone().into(), &mut trigger_integration)
                    .await
                    .expect("Error running Workflow")
            {
                trigger_self.trigger()
            };
            // notify the Cell that the first loop has completed
            if let Some(tx_first) = tx_first.take() {
                let _ = tx_first.send(());
            }

            // Check for shutdown or next job
            let next_job = rx.listen();
            let kill = stop.recv();
            tokio::pin!(next_job);
            tokio::pin!(kill);

            // drop the reader so we don't lock it until the next job!
            drop(reader);

            if let Either::Left((Err(_), _)) | Either::Right((_, _)) =
                futures::future::select(next_job, kill).await
            {
                tracing::warn!("Cell is shutting down: stopping queue consumer.");
                break;
            };
        }
        Ok(())
    });
    (tx, rx_first, handle)
}
