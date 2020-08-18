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
    buffer::KvBuf,
    db::{INTEGRATED_DHT_OPS, INTEGRATION_LIMBO},
    env::EnvironmentWrite,
    error::DatabaseResult,
    prelude::{GetDb, ReadManager, Reader, Writer},
};
use holochain_types::Timestamp;

#[cfg(test)]
mod test;

pub async fn incoming_dht_ops_workflow(
    state_env: &EnvironmentWrite,
    mut sys_validation_trigger: TriggerSender,
    ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
) -> WorkflowResult<()> {
    // set up our workspace
    let env_ref = state_env.guard().await;
    let reader = env_ref.reader()?;
    let mut workspace = IncomingDhtOpsWorkspace::new(&reader, &env_ref)?;

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
        if !workspace.op_exists(&hash)? {
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
pub struct IncomingDhtOpsWorkspace<'env> {
    pub integration_limbo: IntegrationLimboStore<'env>,
    pub integrated_dht_ops: IntegratedDhtOpsStore<'env>,
    pub validation_limbo: ValidationLimboStore<'env>,
}

impl<'env> Workspace<'env> for IncomingDhtOpsWorkspace<'env> {
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
        self.validation_limbo.0.flush_to_txn(writer)?;
        Ok(())
    }
}

impl<'env> IncomingDhtOpsWorkspace<'env> {
    pub fn op_exists(&self, hash: &DhtOpHash) -> DatabaseResult<bool> {
        Ok(self.integrated_dht_ops.contains(&hash)?
            || self.integration_limbo.contains(&hash)?
            || self.validation_limbo.contains(&hash)?)
    }
}
