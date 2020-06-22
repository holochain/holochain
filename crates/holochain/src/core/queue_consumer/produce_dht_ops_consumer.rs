//! The workflow and queue consumer for DhtOp production

use super::*;
use crate::core::{
    state::workspace::Workspace,
    workflow::produce_dht_ops_workflow::{produce_dht_ops_workflow, ProduceDhtOpsWorkspace},
};
use futures::StreamExt;
use holochain_state::env::EnvironmentWrite;
use holochain_state::env::ReadManager;

/// Spawn the QueueConsumer for Produce_dht_ops workflow
pub fn spawn_produce_dht_ops_consumer(
    env: EnvironmentWrite,
    mut trigger_integration: QueueTrigger,
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
                ProduceDhtOpsWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");
            if let WorkComplete::Incomplete =
                produce_dht_ops_workflow(workspace, env.clone().into(), &mut trigger_integration)
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
