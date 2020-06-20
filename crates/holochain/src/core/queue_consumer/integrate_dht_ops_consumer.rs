//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::state::workspace::Workspace;
use crate::core::workflow::integrate_dht_ops_workflow::{
    integrate_dht_ops_workflow, IntegrateDhtOpsWorkspace,
};
use futures::StreamExt;
use holochain_state::env::EnvironmentWrite;
use holochain_state::env::ReadManager;

/// Spawn the QueueConsumer for DhtOpIntegration workflow
pub fn spawn_integrate_dht_ops_consumer(
    env: EnvironmentWrite,
    mut trigger_publish: QueueTrigger,
) -> (QueueTrigger, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = QueueTrigger::new();
    let mut trigger_self = tx.clone();
    let handle = tokio::spawn(async move {
        loop {
            let env_ref = env.guard().await;
            let reader = env_ref.reader().expect("Could not create LMDB reader");
            let workspace = IntegrateDhtOpsWorkspace::new(&reader, &env_ref)
                .expect("Could not create Workspace");
            if let WorkComplete::Incomplete =
                integrate_dht_ops_workflow(workspace, env.clone().into(), &mut trigger_publish)
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
