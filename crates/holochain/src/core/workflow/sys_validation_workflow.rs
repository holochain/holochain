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
    buffer::{BufferedStore, KvBufFresh},
    db::INTEGRATION_LIMBO,
    fresh_reader,
    prelude::*,
};
use holochain_types::validate::ValidationStatus;
use tracing::*;

#[instrument(skip(workspace, writer, trigger_app_validation))]
pub async fn sys_validation_workflow(
    mut workspace: SysValidationWorkspace,
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
    workspace: &mut SysValidationWorkspace,
) -> WorkflowResult<WorkComplete> {
    // one of many ways to get env
    let env = workspace.integration_limbo.env().clone();
    let ops: Vec<ValidationLimboValue> = fresh_reader!(env, |r| workspace
        .validation_limbo
        .drain_iter(&r)?
        .collect())?;
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

pub struct SysValidationWorkspace {
    pub integration_limbo: IntegrationLimboStore,
    pub validation_limbo: ValidationLimboStore,
}

impl SysValidationWorkspace {
    pub fn new(env: EnvironmentRead, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBufFresh::new(env, db);

        let validation_limbo = ValidationLimboStore::new(env, dbs)?;

        Ok(Self {
            integration_limbo,
            validation_limbo,
        })
    }
}

impl Workspace for SysValidationWorkspace {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        warn!("unimplemented passthrough");
        self.validation_limbo.0.flush_to_txn(writer)?;
        self.integration_limbo.flush_to_txn(writer)?;
        Ok(())
    }
}
