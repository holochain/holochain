//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::workspace::{Workspace, WorkspaceResult},
};
use error::WorkflowResult;
use holochain_state::prelude::{GetDb, Reader, Writer};
use tracing::*;

pub async fn integrate_dht_ops_workflow(
    workspace: IntegrateDhtOpsWorkspace<'_>,
    writer: OneshotWriter,
    trigger_publish: &mut TriggerSender,
) -> WorkflowResult<WorkComplete> {
    warn!("unimplemented");

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;

    // trigger other workflows
    trigger_publish.trigger();

    Ok(WorkComplete::Complete)
}

pub struct IntegrateDhtOpsWorkspace<'env>(std::marker::PhantomData<&'env ()>);

impl<'env> Workspace<'env> for IntegrateDhtOpsWorkspace<'env> {
    /// Constructor
    #[allow(dead_code)]
    fn new(_reader: &'env Reader<'env>, _dbs: &impl GetDb) -> WorkspaceResult<Self> {
        Ok(Self(std::marker::PhantomData))
    }
    fn flush_to_txn(self, _writer: &mut Writer) -> WorkspaceResult<()> {
        warn!("unimplemented");
        Ok(())
    }
}
