//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::{
        dht_op_integration::{IntegrationLimboStore, IntegrationLimboValue},
        validation_db::{ValidationLimboStore, ValidationLimboValue},
        workspace::{Workspace, WorkspaceResult},
    },
};
use error::WorkflowResult;
use fallible_iterator::FallibleIterator;
use holo_hash::DhtOpHash;
use holochain_state::{
    buffer::{BufferedStore, KvBuf},
    db::INTEGRATION_LIMBO,
    prelude::{GetDb, Reader, Writer},
};
use holochain_types::validate::ValidationStatus;
use tracing::*;

#[instrument(skip(workspace, writer, trigger_app_validation))]
pub async fn sys_validation_workflow(
    mut workspace: SysValidationWorkspace<'_>,
    writer: OneshotWriter,
    trigger_app_validation: &mut TriggerSender,
) -> WorkflowResult<WorkComplete> {
    tracing::warn!("unimplemented passthrough");
    let complete = sys_validation_workflow_inner(&mut workspace).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))
        .await?;

    // trigger other workflows
    trigger_app_validation.trigger();

    Ok(complete)
}

async fn sys_validation_workflow_inner(
    workspace: &mut SysValidationWorkspace<'_>,
) -> WorkflowResult<WorkComplete> {
    let ops: Vec<ValidationLimboValue> = workspace.validation_limbo.drain_iter()?.collect()?;
    for vlv in ops {
        let op = vlv.op;
        let hash = DhtOpHash::with_data(&op).await;
        let v = IntegrationLimboValue {
            validation_status: ValidationStatus::Valid,
            op,
        };
        workspace.integration_limbo.put(hash, v)?;
    }
    Ok(WorkComplete::Complete)
}

pub struct SysValidationWorkspace<'env> {
    pub integration_limbo: IntegrationLimboStore<'env>,
    pub validation_limbo: ValidationLimboStore<'env>,
}

impl<'env> Workspace<'env> for SysValidationWorkspace<'env> {
    fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBuf::new(reader, db)?;

        let validation_limbo = ValidationLimboStore::new(reader, dbs)?;

        Ok(Self {
            integration_limbo,
            validation_limbo,
        })
    }
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        warn!("unimplemented passthrough");
        self.validation_limbo.0.flush_to_txn(writer)?;
        self.integration_limbo.flush_to_txn(writer)?;
        Ok(())
    }
}
