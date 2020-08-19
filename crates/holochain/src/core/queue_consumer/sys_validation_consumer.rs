//! The workflow and queue consumer for sys validation

use super::*;
use crate::{
    conductor::manager::ManagedTaskResult,
    core::{
        state::workspace::Workspace,
        workflow::sys_validation_workflow::{sys_validation_workflow, SysValidationWorkspace},
    },
};
use futures::future::Either;
use holochain_state::env::EnvironmentWrite;
use holochain_state::env::ReadManager;
use tokio::task::JoinHandle;
use tracing::*;

/// Spawn the QueueConsumer for SysValidation workflow
#[instrument(skip(env, stop, trigger_app_validation, network, conductor_api))]
pub fn spawn_sys_validation_consumer(
    env: EnvironmentWrite,
    mut stop: sync::broadcast::Receiver<()>,
    mut trigger_app_validation: TriggerSender,
    network: HolochainP2pCell,
    conductor_api: impl CellConductorApiT + 'static,
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
                SysValidationWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");
            if let WorkComplete::Incomplete = sys_validation_workflow(
                workspace,
                env.clone().into(),
                &mut trigger_app_validation,
                network.clone(),
                conductor_api.clone(),
            )
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
