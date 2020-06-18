//! The workflow and queue consumer for DhtOp publishing

use super::*;
use crate::core::state::workspace::{Workspace, WorkspaceResult};
use futures::StreamExt;
use holochain_state::env::EnvironmentWrite;
use holochain_state::{
    env::ReadManager,
    prelude::{GetDb, Reader},
};

/// Spawn the QueueConsumer for Publish workflow
pub fn spawn_publish_consumer(
    env: EnvironmentWrite,
    mut rx: QueueTriggerListener,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let env_ref = env.guard().await;
            let reader = env_ref.reader().expect("Could not create LMDB reader");
            let workspace =
                PublishWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");
            publish_workflow(workspace, env.clone().into())
                .await
                .expect("Error running Workflow");
            rx.next().await;
        }
    })
}

struct PublishWorkspace<'env>(std::marker::PhantomData<&'env ()>);

impl<'env> PublishWorkspace<'env> {
    /// Constructor
    #[allow(dead_code)]
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        Ok(Self(std::marker::PhantomData))
    }
}

impl<'env> Workspace<'env> for PublishWorkspace<'env> {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        todo!()
    }
}

async fn publish_workflow<'env>(
    workspace: PublishWorkspace<'env>,
    writer: OneshotWriter,
) -> anyhow::Result<()> {
    todo!("implement workflow");

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;

    // trigger other workflows
    // (n/a)

    Ok(())
}
