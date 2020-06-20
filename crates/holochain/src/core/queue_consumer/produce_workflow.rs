//! The workflow and queue consumer for DhtOp production

use super::*;
use crate::core::{
    state::workspace::{Workspace, WorkspaceResult},
    workflow::{
        error::WorkflowRunResult,
        produce_dht_op_workflow::{produce_dht_op_workflow, ProduceDhtOpWorkspace},
    },
};
use futures::StreamExt;
use holochain_state::env::EnvironmentWrite;
use holochain_state::{
    env::ReadManager,
    prelude::{GetDb, Reader},
};

/// Spawn the QueueConsumer for Produce workflow
pub fn spawn_produce_consumer(
    env: EnvironmentWrite,
    mut trigger_integration: QueueTrigger,
) -> (QueueTrigger, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = QueueTrigger::new();
    let mut trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            let env_ref = env.guard().await;
            let reader = env_ref.reader().expect("Could not create LMDB reader");
            let workspace =
                ProduceDhtOpWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");
            if let WorkComplete::Incomplete =
                produce_dht_op_workflow(workspace, env.clone().into(), &mut trigger_integration)
                    .await
                    .expect("Error running Workflow")
            {
                trigger_self.trigger().expect("Trigger channel closed")
            };
            rx.next().await;
        }
    });
    (tx, handle)
}
