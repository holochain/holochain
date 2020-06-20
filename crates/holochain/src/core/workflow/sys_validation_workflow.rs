//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::{
    queue_consumer::{OneshotWriter, QueueTrigger, WorkComplete},
    state::workspace::{Workspace, WorkspaceResult},
};
use error::WorkflowResult;
use holochain_state::prelude::{GetDb, Reader, Writer};

pub async fn sys_validation_workflow<'env>(
    workspace: SysValidationWorkspace<'env>,
    writer: OneshotWriter,
    trigger_app_validation: &mut QueueTrigger,
) -> WorkflowResult<WorkComplete> {
    todo!("implement workflow");

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;

    // trigger other workflows
    trigger_app_validation.trigger();

    Ok(WorkComplete::Complete)
}

pub struct SysValidationWorkspace<'env>(std::marker::PhantomData<&'env ()>);

impl<'env> SysValidationWorkspace<'env> {}

impl<'env> Workspace<'env> for SysValidationWorkspace<'env> {
    /// Constructor
    #[allow(dead_code)]
    fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        Ok(Self(std::marker::PhantomData))
    }
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        todo!()
    }
}
