//! The workflow and queue consumer for sys validation

use super::error::WorkflowResult;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::{
        dht_op_integration::{IntegratedDhtOpsStore, IntegrationLimboStore, IntegrationLimboValue},
        validation_db::{ValidationLimboStatus, ValidationLimboStore, ValidationLimboValue},
        workspace::{Workspace, WorkspaceResult},
    },
};
use fallible_iterator::FallibleIterator;
use holo_hash::DhtOpHash;
use holochain_state::{
    buffer::{BufferedStore, KvBuf},
    db::{INTEGRATED_DHT_OPS, INTEGRATION_LIMBO},
    prelude::{GetDb, Reader, Writer},
};
use holochain_types::validate::ValidationStatus;
use tracing::*;

#[instrument(skip(workspace, writer, trigger_integration))]
pub async fn app_validation_workflow(
    mut workspace: AppValidationWorkspace<'_>,
    writer: OneshotWriter,
    trigger_integration: &mut TriggerSender,
) -> WorkflowResult<WorkComplete> {
    warn!("unimplemented passthrough");

    let complete = app_validation_workflow_inner(&mut workspace).await?;
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))
        .await?;

    // trigger other workflows
    trigger_integration.trigger();

    Ok(complete)
}
async fn app_validation_workflow_inner(
    workspace: &mut AppValidationWorkspace<'_>,
) -> WorkflowResult<WorkComplete> {
    let ops: Vec<ValidationLimboValue> = workspace
        .validation_limbo
        .drain_iter()?
        .filter(|vlv| {
            match vlv.status {
                // We only want sys validated or awaiting app dependency ops
                ValidationLimboStatus::SysValidated | ValidationLimboStatus::AwaitingAppDeps(_) => {
                    Ok(true)
                }
                ValidationLimboStatus::Pending | ValidationLimboStatus::AwaitingSysDeps(_) => {
                    Ok(false)
                }
            }
        })
        .collect()?;
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

pub struct AppValidationWorkspace<'env> {
    pub integration_limbo: IntegrationLimboStore<'env>,
    pub integrated_dht_ops: IntegratedDhtOpsStore<'env>,
    pub validation_limbo: ValidationLimboStore<'env>,
}

impl<'env> Workspace<'env> for AppValidationWorkspace<'env> {
    fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBuf::new(reader, db)?;

        let db = dbs.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBuf::new(reader, db)?;

        let validation_limbo = ValidationLimboStore::new(reader, dbs)?;

        Ok(Self {
            integration_limbo,
            integrated_dht_ops,
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
