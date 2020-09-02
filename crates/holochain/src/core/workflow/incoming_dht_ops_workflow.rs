//! The workflow and queue consumer for DhtOp integration

use super::{
    error::WorkflowResult,
    integrate_dht_ops_workflow::{integrate_single_metadata, integrate_single_op},
    produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertResult,
    sys_validation_workflow::types::Dependencies,
};
use crate::core::{
    queue_consumer::TriggerSender,
    state::{
        dht_op_integration::{IntegratedDhtOpsStore, IntegrationLimboStore},
        element_buf::ElementBuf,
        metadata::MetadataBuf,
        validation_db::{ValidationLimboStatus, ValidationLimboStore, ValidationLimboValue},
        workspace::{Workspace, WorkspaceResult},
    },
};
use holo_hash::DhtOpHash;
use holochain_state::{
    buffer::BufferedStore,
    buffer::KvBufFresh,
    db::{INTEGRATED_DHT_OPS, INTEGRATION_LIMBO},
    env::EnvironmentWrite,
    error::DatabaseResult,
    prelude::{EnvironmentRead, GetDb, PendingPrefix, Writer},
};
use holochain_types::{dht_op::DhtOp, Timestamp};
use tracing::instrument;

#[cfg(test)]
mod test;

#[instrument(skip(state_env, sys_validation_trigger, ops))]
pub async fn incoming_dht_ops_workflow(
    state_env: &EnvironmentWrite,
    mut sys_validation_trigger: TriggerSender,
    ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
) -> WorkflowResult<()> {
    // set up our workspace
    let env_ref = state_env.guard();
    let mut workspace = IncomingDhtOpsWorkspace::new(state_env.clone().into(), &env_ref)?;

    // add incoming ops to the validation limbo
    for (hash, op) in ops {
        if !workspace.op_exists(&hash)? {
            tracing::debug!(?op);
            workspace.add_to_pending(hash, op).await?;
        }
    }

    // commit our transaction
    let writer: crate::core::queue_consumer::OneshotWriter = state_env.clone().into();

    writer.with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))?;

    // trigger validation of queued ops
    sys_validation_trigger.trigger();

    Ok(())
}

#[allow(missing_docs)]
pub struct IncomingDhtOpsWorkspace {
    pub integration_limbo: IntegrationLimboStore,
    pub integrated_dht_ops: IntegratedDhtOpsStore,
    pub validation_limbo: ValidationLimboStore,
    pub element_pending: ElementBuf<PendingPrefix>,
    pub meta_pending: MetadataBuf<PendingPrefix>,
}

impl Workspace for IncomingDhtOpsWorkspace {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.validation_limbo.0.flush_to_txn(writer)?;
        self.element_pending.flush_to_txn(writer)?;
        self.meta_pending.flush_to_txn(writer)?;
        Ok(())
    }
}

impl IncomingDhtOpsWorkspace {
    pub fn new(env: EnvironmentRead, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBufFresh::new(env.clone(), db);

        let db = dbs.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBufFresh::new(env.clone(), db);

        let validation_limbo = ValidationLimboStore::new(env.clone(), dbs)?;

        let element_pending = ElementBuf::pending(env.clone(), dbs)?;
        let meta_pending = MetadataBuf::pending(env, dbs)?;

        Ok(Self {
            integration_limbo,
            integrated_dht_ops,
            validation_limbo,
            element_pending,
            meta_pending,
        })
    }

    async fn add_to_pending(&mut self, hash: DhtOpHash, op: DhtOp) -> DhtOpConvertResult<()> {
        let basis = op.dht_basis().await;
        let op_light = op.to_light().await;

        integrate_single_op(op, &mut self.element_pending).await?;
        integrate_single_metadata(
            op_light.clone(),
            &self.element_pending,
            &mut self.meta_pending,
        )
        .await?;
        let vlv = ValidationLimboValue {
            status: ValidationLimboStatus::Pending,
            op: op_light,
            basis,
            time_added: Timestamp::now(),
            last_try: None,
            num_tries: 0,
            awaiting_proof: Dependencies::new(),
        };
        self.validation_limbo.put(hash, vlv)?;
        Ok(())
    }

    pub fn op_exists(&self, hash: &DhtOpHash) -> DatabaseResult<bool> {
        Ok(self.integrated_dht_ops.contains(&hash)?
            || self.integration_limbo.contains(&hash)?
            || self.validation_limbo.contains(&hash)?)
    }
}
