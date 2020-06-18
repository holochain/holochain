//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::state::workspace::{Workspace, WorkspaceResult};
use futures::StreamExt;
use holochain_state::env::EnvironmentWrite;
use holochain_state::{
    env::ReadManager,
    prelude::{GetDb, Reader},
};

/// Spawn the QueueConsumer for SysValidation workflow
pub fn spawn_sys_validation_consumer(
    env: EnvironmentWrite,
    mut rx: QueueTriggerListener,
    mut trigger_app_validation: QueueTrigger,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let env_ref = env.guard().await;
            let reader = env_ref.reader().expect("Could not create LMDB reader");
            let workspace =
                SysValidationWorkspace::new(&reader, &env_ref).expect("Could not create Workspace");
            sys_validation_workflow(workspace, env.clone().into(), &mut trigger_app_validation)
                .await
                .expect("Error running Workflow");
            rx.next().await;
        }
    })
}

struct SysValidationWorkspace<'env>(std::marker::PhantomData<&'env ()>);

impl<'env> SysValidationWorkspace<'env> {
    /// Constructor
    #[allow(dead_code)]
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        Ok(Self(std::marker::PhantomData))
    }
}

impl<'env> Workspace<'env> for SysValidationWorkspace<'env> {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        todo!()
    }
}

async fn sys_validation_workflow<'env>(
    workspace: SysValidationWorkspace<'env>,
    writer: OneshotWriter,
    trigger_app_validation: &mut QueueTrigger,
) -> anyhow::Result<()> {
    todo!("implement workflow");

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;

    // trigger other workflows
    trigger_app_validation.trigger();

    Ok(())
}
