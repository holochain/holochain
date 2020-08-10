//! The workflow and queue consumer for sys validation

use super::error::WorkflowResult;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::workspace::{Workspace, WorkspaceResult},
};
use holochain_state::prelude::{GetDb, Reader, Writer};
use tracing::*;

#[instrument(skip(workspace, writer, trigger_integration))]
pub async fn app_validation_workflow(
    workspace: AppValidationWorkspace<'_>,
    writer: OneshotWriter,
    trigger_integration: &mut TriggerSender,
) -> WorkflowResult<WorkComplete> {
    warn!("unimplemented");

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;

    // trigger other workflows
    trigger_integration.trigger();

    Ok(WorkComplete::Complete)
}

pub struct AppValidationWorkspace<'env>(std::marker::PhantomData<&'env ()>);

impl<'env> AppValidationWorkspace<'env> {}

impl<'env> Workspace<'env> for AppValidationWorkspace<'env> {
    /// Constructor
    fn new(_reader: &'env Reader<'env>, _dbs: &impl GetDb) -> WorkspaceResult<Self> {
        Ok(Self(std::marker::PhantomData))
    }

    fn flush_to_txn(self, _writer: &mut Writer) -> WorkspaceResult<()> {
        warn!("unimplemented");
        Ok(())
    }
}
