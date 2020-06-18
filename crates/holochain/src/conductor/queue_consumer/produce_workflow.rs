//! The workflow and queue consumer for DhtOp production

use super::*;
use crate::core::state::workspace::{Workspace, WorkspaceResult};
use futures::StreamExt;
use holochain_state::env::EnvironmentWrite;
use holochain_state::{
    env::ReadManager,
    prelude::{GetDb, Reader},
};

/// Spawn the QueueConsumer for Produce workflow
pub fn spawn_produce_consumer(
    env: EnvironmentWrite,
    mut rx: QueueTriggerListener,
    mut trigger_integration: QueueTrigger,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let env_ref = env.guard().await;
            let reader = env_ref.reader().expect("Could not create LMDB reader");
            let workspace =
                ProduceWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");
            produce_workflow(workspace, env.clone().into(), &mut trigger_integration)
                .await
                .expect("Error running Workflow");
            rx.next().await;
        }
    })
}

struct ProduceWorkspace<'env>(std::marker::PhantomData<&'env ()>);

impl<'env> ProduceWorkspace<'env> {
    /// Constructor
    #[allow(dead_code)]
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        Ok(Self(std::marker::PhantomData))
    }
}

impl<'env> Workspace<'env> for ProduceWorkspace<'env> {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        todo!()
    }
}

async fn produce_workflow<'env>(
    workspace: ProduceWorkspace<'env>,
    writer: OneshotWriter,
    trigger_integration: &mut QueueTrigger,
) -> anyhow::Result<()> {
    todo!("implement workflow");

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;

    // trigger other workflows
    trigger_integration.trigger();

    Ok(())
}
