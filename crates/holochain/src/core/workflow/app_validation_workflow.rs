//! The workflow and queue consumer for sys validation

use super::error::WorkflowResult;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::{
        dht_op_integration::{IntegratedDhtOpsStore, IntegrationLimboStore},
        validation_db::ValidationLimboStore,
        workspace::{Workspace, WorkspaceResult},
    },
};
use holochain_state::{
    buffer::{BufferedStore, KvBuf},
    db::{INTEGRATED_DHT_OPS, INTEGRATION_LIMBO},
    prelude::{GetDb, Reader, Writer},
};
use tracing::*;

#[instrument(skip(workspace, writer, trigger_integration))]
pub async fn app_validation_workflow(
    workspace: AppValidationWorkspace<'_>,
    writer: OneshotWriter,
    trigger_integration: &mut TriggerSender,
) -> WorkflowResult<WorkComplete> {
    warn!("unimplemented passthrough");

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))
        .await?;

    // trigger other workflows
    trigger_integration.trigger();

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
