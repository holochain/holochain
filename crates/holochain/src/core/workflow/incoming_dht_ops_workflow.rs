//! The workflow and queue consumer for DhtOp integration

use super::error::WorkflowResult;
use crate::core::{
    queue_consumer::TriggerSender,
    state::{
        dht_op_integration::{IntegratedDhtOpsStore, IntegrationLimboStore},
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
    prelude::{EnvironmentRead, GetDb, ReadManager, Writer},
};
use holochain_types::Timestamp;
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
    let env_ref = state_env.guard().await;
    let _reader = env_ref.reader()?;
    let mut workspace = IncomingDhtOpsWorkspace::new(state_env.clone().into(), &env_ref)?;

    // add incoming ops to the validation limbo
    for (hash, op) in ops {
        let basis = op.dht_basis().await;
        let vqv = ValidationLimboValue {
            status: ValidationLimboStatus::Pending,
            op,
            basis,
            time_added: Timestamp::now(),
            last_try: None,
            num_tries: 0,
        };
        if !workspace.op_exists(&hash).await? {
            workspace.validation_limbo.put(hash, vqv)?;
        }
    }

    // commit our transaction
    let writer: crate::core::queue_consumer::OneshotWriter = state_env.clone().into();

    writer
        .with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))
        .await?;

    // trigger validation of queued ops
    sys_validation_trigger.trigger();

    Ok(())
}

#[allow(missing_docs)]
pub struct IncomingDhtOpsWorkspace {
    pub integration_limbo: IntegrationLimboStore,
    pub integrated_dht_ops: IntegratedDhtOpsStore,
    pub validation_limbo: ValidationLimboStore,
}

impl Workspace for IncomingDhtOpsWorkspace {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.validation_limbo.0.flush_to_txn(writer)?;
        Ok(())
    }
}

impl IncomingDhtOpsWorkspace {
    pub fn new(env: EnvironmentRead, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBufFresh::new(env.clone(), db);

        let db = dbs.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBufFresh::new(env.clone(), db);

        let validation_limbo = ValidationLimboStore::new(env, dbs)?;

        Ok(Self {
            integration_limbo,
            integrated_dht_ops,
            validation_limbo,
        })
    }

    pub async fn op_exists(&self, hash: &DhtOpHash) -> DatabaseResult<bool> {
        Ok(self.integrated_dht_ops.contains(&hash).await?
            || self.integration_limbo.contains(&hash).await?
            || self.validation_limbo.contains(&hash).await?)
    }
}
