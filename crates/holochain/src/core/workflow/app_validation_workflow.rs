//! The workflow and queue consumer for sys validation

use super::{error::WorkflowResult, produce_dht_ops_workflow::dht_op_light::light_to_op};
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::{
        dht_op_integration::{IntegratedDhtOpsStore, IntegrationLimboStore, IntegrationLimboValue},
        element_buf::ElementBuf,
        metadata::MetadataBuf,
        validation_db::{ValidationLimboStatus, ValidationLimboStore, ValidationLimboValue},
        workspace::{Workspace, WorkspaceResult},
    },
};
use fallible_iterator::FallibleIterator;
use holo_hash::DhtOpHash;
use holochain_state::{
    buffer::{BufferedStore, KvBufFresh},
    db::{INTEGRATED_DHT_OPS, INTEGRATION_LIMBO},
    fresh_reader,
    prelude::*,
};
use holochain_types::{dht_op::DhtOp, validate::ValidationStatus, Timestamp};
use tracing::*;

#[instrument(skip(workspace, writer, trigger_integration))]
pub async fn app_validation_workflow(
    mut workspace: AppValidationWorkspace,
    writer: OneshotWriter,
    trigger_integration: &mut TriggerSender,
) -> WorkflowResult<WorkComplete> {
    warn!("unimplemented passthrough");

    let complete = app_validation_workflow_inner(&mut workspace).await?;
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer.with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))?;

    // trigger other workflows
    trigger_integration.trigger();

    Ok(complete)
}
async fn app_validation_workflow_inner(
    workspace: &mut AppValidationWorkspace,
) -> WorkflowResult<WorkComplete> {
    let env = workspace.validation_limbo.env().clone();
    let ops: Vec<ValidationLimboValue> = fresh_reader!(env, |r| workspace
        .validation_limbo
        .drain_iter_filter(&r, |(_, vlv)| {
            match vlv.status {
                // We only want sys validated or awaiting app dependency ops
                ValidationLimboStatus::SysValidated | ValidationLimboStatus::AwaitingAppDeps(_) => {
                    Ok(true)
                }
                ValidationLimboStatus::Pending | ValidationLimboStatus::AwaitingSysDeps(_) => {
                    Ok(false)
                }
            }
        })?
        .collect())?;

    for vlv in ops {
        match &vlv.status {
            ValidationLimboStatus::AwaitingAppDeps(_) => {
                let op = light_to_op(vlv.op.clone(), &workspace.element_pending)?;
                let hash = DhtOpHash::with_data_sync(&op);
                workspace.put_val_limbo(hash, vlv)?;
            }
            ValidationLimboStatus::SysValidated => {
                let op = light_to_op(vlv.op.clone(), &workspace.element_pending)?;
                let hash = DhtOpHash::with_data_sync(&op);
                let iv = IntegrationLimboValue {
                    validation_status: ValidationStatus::Valid,
                    op: vlv.op,
                };
                workspace.put_int_limbo(hash, iv, op)?;
            }
            _ => unreachable!("Should not contain any other status"),
        }
    }
    Ok(WorkComplete::Complete)
}

pub struct AppValidationWorkspace {
    pub integrated_dht_ops: IntegratedDhtOpsStore,
    pub integration_limbo: IntegrationLimboStore,
    pub validation_limbo: ValidationLimboStore,
    // Integrated data
    pub element_vault: ElementBuf,
    pub meta_vault: MetadataBuf,
    // Data pending validation
    pub element_pending: ElementBuf<PendingPrefix>,
    pub meta_pending: MetadataBuf<PendingPrefix>,
    // Cached data
    pub element_cache: ElementBuf,
    pub meta_cache: MetadataBuf,
}

impl AppValidationWorkspace {
    pub fn new(env: EnvironmentRead) -> WorkspaceResult<Self> {
        let db = env.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBufFresh::new(env.clone(), db);
        let db = env.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBufFresh::new(env.clone(), db);

        let validation_limbo = ValidationLimboStore::new(env.clone())?;

        let element_vault = ElementBuf::vault(env.clone(), false)?;
        let meta_vault = MetadataBuf::vault(env.clone())?;
        let element_cache = ElementBuf::cache(env.clone())?;
        let meta_cache = MetadataBuf::cache(env.clone())?;

        let element_pending = ElementBuf::pending(env.clone())?;
        let meta_pending = MetadataBuf::pending(env)?;

        Ok(Self {
            integrated_dht_ops,
            integration_limbo,
            validation_limbo,
            element_vault,
            meta_vault,
            element_pending,
            meta_pending,
            element_cache,
            meta_cache,
        })
    }

    fn put_val_limbo(
        &mut self,
        hash: DhtOpHash,
        mut vlv: ValidationLimboValue,
    ) -> WorkflowResult<()> {
        vlv.last_try = Some(Timestamp::now());
        vlv.num_tries += 1;
        self.validation_limbo.put(hash, vlv)?;
        Ok(())
    }

    #[tracing::instrument(skip(self, hash))]
    fn put_int_limbo(
        &mut self,
        hash: DhtOpHash,
        iv: IntegrationLimboValue,
        op: DhtOp,
    ) -> WorkflowResult<()> {
        self.integration_limbo.put(hash, iv)?;
        Ok(())
    }
}

impl Workspace for AppValidationWorkspace {
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        warn!("unimplemented passthrough");
        self.validation_limbo.0.flush_to_txn_ref(writer)?;
        self.integration_limbo.flush_to_txn_ref(writer)?;
        self.element_pending.flush_to_txn_ref(writer)?;
        self.meta_pending.flush_to_txn_ref(writer)?;
        Ok(())
    }
}
