//! The workflow and queue consumer for sys validation

use super::error::WorkflowResult;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::{
        dht_op_integration::IntegrationLimboStore,
        validation_db::ValidationLimboStore,
        workspace::{Workspace, WorkspaceResult},
    },
};
use holochain_state::{
    buffer::{BufferedStore, KvBufFresh},
    db::INTEGRATION_LIMBO,
    prelude::*,
};
use tracing::*;

#[instrument(skip(workspace, writer, trigger_integration))]
pub async fn app_validation_workflow(
    workspace: AppValidationWorkspace,
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

pub struct AppValidationWorkspace {
    pub integration_limbo: IntegrationLimboStore,
    pub validation_limbo: ValidationLimboStore,
}

impl AppValidationWorkspace {
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

impl Workspace for AppValidationWorkspace {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        warn!("unimplemented passthrough");
        self.validation_limbo.0.flush_to_txn(writer)?;
        self.integration_limbo.flush_to_txn(writer)?;
        Ok(())
    }
}
