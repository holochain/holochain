use super::error::WorkflowError;
use fallible_iterator::FallibleIterator;
use holo_hash::DhtOpHash;
use holochain_cascade::Cascade;
use holochain_cascade::DbPair;
use holochain_keystore::KeystoreSender;
use holochain_lmdb::buffer::KvBufFresh;
use holochain_lmdb::db::GetDb;
use holochain_lmdb::db::INTEGRATED_DHT_OPS;
use holochain_lmdb::env::EnvironmentRead;
use holochain_lmdb::fresh_reader;
use holochain_lmdb::prelude::*;
use holochain_p2p::HolochainP2pCell;
use holochain_p2p::HolochainP2pCellT;
use holochain_state::prelude::*;
use holochain_zome_types::TryInto;
use tracing::*;

use crate::core::queue_consumer::OneshotWriter;
use crate::core::queue_consumer::WorkComplete;

use super::error::WorkflowResult;

#[cfg(test)]
mod tests;

#[instrument(skip(workspace, writer, network))]
/// Send validation receipts to their authors in serial and without waiting for
/// responses.
/// TODO: Currently still waiting for responses because we don't have a network call
/// that doesn't.
pub async fn validation_receipt_workflow(
    mut workspace: ValidationReceiptWorkspace,
    writer: OneshotWriter,
    network: &mut HolochainP2pCell,
) -> WorkflowResult<WorkComplete> {
    // Get the env and keystore
    let env = workspace.elements.headers().env().clone();
    let keystore = workspace.keystore.clone();

    // Get out all ops that are marked for sending receipt.
    let ops: Vec<(DhtOpHash, IntegratedDhtOpsValue)> = fresh_reader!(env, |r| workspace
        .integrated_dht_ops
        .iter(&r)?
        .filter(|(_, v)| Ok(v.send_receipt))
        .map_err(WorkflowError::from)
        .map(|(k, v)| Ok((DhtOpHash::from_raw_39(k.to_vec())?, v)))
        .collect())?;

    // Who we are.
    let agent = network.from_agent();

    // Send the validation receipts
    for (dht_op_hash, mut op) in ops {
        // Get the header so we know who to send it to.
        let header = {
            // Don't worry this cascade is constructed without a network so
            // it's all local.
            let mut cascade = workspace.cascade();
            cascade
                .retrieve_header(op.op.header_hash().clone(), Default::default())
                .await?
        };
        let to_agent = match header {
            Some(header) => header.header().author().clone(),
            None => {
                // Not sure why we have an op but not the data to go with it.
                warn!(op_missing_data_for_receipt = ?op);
                continue;
            }
        };

        // Don't send receipt to self.
        if to_agent == agent {
            continue;
        }

        // Create the receipt.
        let receipt = ValidationReceipt {
            dht_op_hash: dht_op_hash.clone(),
            validation_status: op.validation_status,
            validator: agent.clone(),
            when_integrated: op.when_integrated,
        };

        // Sign on the dotted line.
        let receipt = receipt.sign(&keystore).await?;

        // Send it and don't wait for response.
        // TODO: When networking has a send without response we can use that
        // instead of waiting for response.
        if let Err(e) = network
            .send_validation_receipt(to_agent, receipt.try_into()?)
            .await
        {
            // No one home, they will need to publish again.
            info!(failed_send_receipt = ?e);
        }
        // Attempted to send the receipt so we now mark
        // it to not send in the future.
        op.send_receipt = false;
        workspace.integrated_dht_ops.put(dht_op_hash, op)?;
    }

    // Write the acknowledgment to the db.
    writer.with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))?;

    Ok(WorkComplete::Complete)
}

pub struct ValidationReceiptWorkspace {
    // Get the ops from here:
    pub integrated_dht_ops: IntegratedDhtOpsStore,

    // Find the author in here:
    pub elements: ElementBuf,
    // TODO: Probs don't need the meta store but seeing
    // as SQL is coming this just makes using the cascade easier.
    pub meta: MetadataBuf,
    pub element_rejected: ElementBuf<RejectedPrefix>,
    pub meta_rejected: MetadataBuf<RejectedPrefix>,

    // Sign receipts with this:
    pub keystore: KeystoreSender,
}

impl ValidationReceiptWorkspace {
    /// Make a new workspace.
    pub fn new(env: EnvironmentRead) -> WorkspaceResult<Self> {
        let keystore = env.keystore().clone();
        let db = env.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBufFresh::new(env.clone(), db);

        let elements = ElementBuf::vault(env.clone(), true)?;
        let meta = MetadataBuf::vault(env.clone())?;

        let element_rejected = ElementBuf::rejected(env.clone())?;
        let meta_rejected = MetadataBuf::rejected(env)?;

        Ok(Self {
            integrated_dht_ops,
            elements,
            meta,
            element_rejected,
            meta_rejected,
            keystore,
        })
    }

    /// Create a local only cascade.
    pub fn cascade(&self) -> Cascade<'_> {
        let integrated_data = DbPair {
            element: &self.elements,
            meta: &self.meta,
        };
        let rejected_data = DbPair {
            element: &self.element_rejected,
            meta: &self.meta_rejected,
        };
        Cascade::empty()
            .with_integrated(integrated_data)
            .with_rejected(rejected_data)
    }
}

impl Workspace for ValidationReceiptWorkspace {
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        // Only writing to the integrated ops table. Rest is read only.
        self.integrated_dht_ops.flush_to_txn_ref(writer)?;
        Ok(())
    }
}
