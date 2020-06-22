//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::{
    state::workspace::Workspace,
    workflow::sys_validation_workflow::{sys_validation_workflow, SysValidationWorkspace},
};
use futures::StreamExt;
use holochain_state::env::EnvironmentWrite;
use holochain_state::env::ReadManager;

/// Spawn the QueueConsumer for SysValidation workflow
pub fn spawn_sys_validation_consumer(
    env: EnvironmentWrite,
    mut trigger_app_validation: QueueTrigger,
) -> (QueueTrigger, tokio::sync::mpsc::Receiver<()>) {
    let (tx, mut rx) = QueueTrigger::new();
    let (tx_first, rx_first) = tokio::sync::mpsc::channel(1);
    let mut tx_first = Some(tx_first);
    let mut trigger_self = tx.clone();
    let _handle = tokio::spawn(async move {
        loop {
            let env_ref = env.guard().await;
            let reader = env_ref.reader().expect("Could not create LMDB reader");
            let workspace =
                SysValidationWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");
            if let WorkComplete::Incomplete =
                sys_validation_workflow(workspace, env.clone().into(), &mut trigger_app_validation)
                    .await
                    .expect("Error running Workflow")
            {
                trigger_self.trigger().expect("Trigger channel closed")
            };
            if let Some(mut tx_first) = tx_first.take() {
                let _ = tx_first.send(());
            }
            rx.next().await;
        }
    });
    (tx, rx_first)
}
