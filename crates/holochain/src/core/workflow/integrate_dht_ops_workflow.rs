//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::{
        chain_cas::ChainCasBuf,
        dht_op_integration::{
            IntegratedDhtOpsStore, IntegratedDhtOpsValue, IntegrationQueueStore,
            IntegrationQueueValue,
        },
        metadata::{LinkMetaKey, MetadataBuf, MetadataBufT},
        workspace::{Workspace, WorkspaceResult},
    },
};
use error::WorkflowResult;
use fallible_iterator::FallibleIterator;
use holo_hash::{AgentPubKey, Hashable, Hashed};
use holochain_state::{
    buffer::BufferedStore,
    buffer::KvBuf,
    db::{INTEGRATED_DHT_OPS, INTEGRATION_QUEUE},
    prelude::{GetDb, Reader, Writer},
};
use holochain_types::{
    dht_op::{DhtOp, DhtOpHashed},
    element::SignedHeaderHashed,
    header::IntendedFor,
    EntryHashed, Header, HeaderHashed, Timestamp,
};
use produce_dht_ops_workflow::dht_op_light::{dht_op_to_light_basis, error::DhtOpConvertError};
use std::convert::TryInto;
use tracing::*;

mod tests;

pub async fn integrate_dht_ops_workflow(
    mut workspace: IntegrateDhtOpsWorkspace<'_>,
    writer: OneshotWriter,
    trigger_publish: &mut TriggerSender,
    agent_pub_key: AgentPubKey,
) -> WorkflowResult<WorkComplete> {
    let result = integrate_dht_ops_workflow_inner(&mut workspace, agent_pub_key).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;

    // trigger other workflows
    // TODO: only trigger if we have integrated ops that we have authored
    trigger_publish.trigger();

    Ok(result)
}

#[instrument(skip(workspace))]
async fn integrate_dht_ops_workflow_inner(
    workspace: &mut IntegrateDhtOpsWorkspace<'_>,
    agent_pub_key: AgentPubKey,
) -> WorkflowResult<WorkComplete> {
    debug!("Starting integrate dht ops workflow");
    // Pull ops out of queue
    // TODO: PERF: Not collect, iterator cannot cross awaits
    // Find a way to do this.
    let ops = workspace
        .integration_queue
        .drain_iter_reverse()?
        .collect::<Vec<_>>()?;

    for value in ops {
        // Process each op
        let IntegrationQueueValue {
            op,
            validation_status,
        } = value;

        let (op, op_hash) = DhtOpHashed::with_data(op).await.into_inner();
        debug!(?op_hash);
        debug!(?op);

        // TODO: PERF: We don't really need this clone because dht_to_op_light_basis could
        // return the full op as it's not consumed when making hashes

        match op.clone() {
            DhtOp::StoreElement(signature, header, maybe_entry) => {
                let header = HeaderHashed::with_data(header).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                let entry_hashed = match maybe_entry {
                    Some(entry) => Some(EntryHashed::with_data(*entry).await?),
                    None => None,
                };
                // Store the entry
                workspace.cas.put(signed_header, entry_hashed)?;
            }
            DhtOp::StoreEntry(signature, new_entry_header, entry) => {
                // Reference to headers
                workspace
                    .meta
                    .register_header(new_entry_header.clone())
                    .await?;

                let header = HeaderHashed::with_data(new_entry_header.into()).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                let entry = EntryHashed::with_data(*entry).await?;
                // Store Header and Entry
                workspace.cas.put(signed_header, Some(entry))?;
            }
            DhtOp::RegisterAgentActivity(signature, header) => {
                // Store header
                let header_hashed = HeaderHashed::with_data(header.clone()).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header_hashed, signature);
                workspace.cas.put(signed_header, None)?;

                // register agent activity on this agents pub key
                workspace
                    .meta
                    .register_activity(header, agent_pub_key.clone())
                    .await?;
            }
            DhtOp::RegisterReplacedBy(_, entry_update, _) => {
                let old_entry_hash = match entry_update.intended_for {
                    IntendedFor::Header => None,
                    IntendedFor::Entry => {
                        match workspace
                            .cas
                            .get_header(&entry_update.replaces_address)
                            .await?
                            // Handle missing old entry header. Same reason as below
                            .and_then(|e| e.header().entry_data().map(|(hash, _)| hash.clone()))
                        {
                            Some(e) => Some(e),
                            // Handle missing old Entry (Probably StoreEntry hasn't arrived been processed)
                            // This is put the op back in the integration queue to try again later
                            None => {
                                workspace.integration_queue.put(
                                    (Timestamp::now(), op_hash).try_into()?,
                                    IntegrationQueueValue {
                                        validation_status,
                                        op,
                                    },
                                )?;
                                continue;
                            }
                        }
                    }
                };
                workspace
                    .meta
                    .register_update(entry_update, old_entry_hash)
                    .await?;
            }
            DhtOp::RegisterDeletedEntryHeader(_, entry_delete) => {
                let entry_hash = match workspace
                    .cas
                    .get_header(&entry_delete.removes_address)
                    .await?
                    // Handle missing entry header. Same reason as below
                    .and_then(|e| e.header().entry_data().map(|(hash, _)| hash.clone()))
                {
                    Some(e) => e,
                    // TODO: VALIDATION: This could also be an invalid delete on a header without a delete
                    // Handle missing Entry (Probably StoreEntry hasn't arrived been processed)
                    // This is put the op back in the integration queue to try again later
                    None => {
                        workspace.integration_queue.put(
                            (Timestamp::now(), op_hash).try_into()?,
                            IntegrationQueueValue {
                                validation_status,
                                op,
                            },
                        )?;
                        continue;
                    }
                };
                workspace
                    .meta
                    .register_delete_on_entry(entry_delete, entry_hash)
                    .await?
            }
            DhtOp::RegisterDeletedBy(_, entry_delete) => {
                workspace
                    .meta
                    .register_delete_on_header(entry_delete)
                    .await?
            }
            DhtOp::RegisterAddLink(signature, link_add) => {
                workspace.meta.add_link(link_add.clone()).await?;
                // Store add Header
                let header = HeaderHashed::with_data(link_add.into()).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                workspace.cas.put(signed_header, None)?;
            }
            DhtOp::RegisterRemoveLink(signature, link_remove) => {
                // Check whether they have the base address in the cas.
                // If not then this should put the op back on the queue with a
                // warning that it's unimplemented and later add this to the cache meta.
                // TODO: Base might be in cas due to this agent being an authority for a
                // header on the Base
                if let None = workspace.cas.get_entry(&link_remove.base_address).await? {
                    warn!(
                        "Storing link data when not an author or authority requires the
                         cache metadata store.
                         The cache metadata store is currently unimplemented"
                    );
                    // Add op back on queue
                    workspace.integration_queue.put(
                        (Timestamp::now(), op_hash).try_into()?,
                        IntegrationQueueValue {
                            validation_status,
                            op,
                        },
                    )?;
                    continue;
                }

                // Store link delete Header
                let header = HeaderHashed::with_data(link_remove.clone().into()).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                workspace.cas.put(signed_header, None)?;

                // Get the link add header
                let maybe_link_add = match workspace
                    .cas
                    .get_header(&link_remove.link_add_address)
                    .await?
                {
                    Some(link_add) => {
                        let header = link_add.into_header_and_signature().0;
                        let (header, hash) = header.into_inner();
                        let link_add = match header {
                            Header::LinkAdd(la) => la,
                            _ => return Err(DhtOpConvertError::LinkRemoveRequiresLinkAdd.into()),
                        };

                        // Create a full link key and check if the link add exists
                        let key = LinkMetaKey::from((&link_add, &hash));
                        if workspace.meta.get_links(&key)?.is_empty() {
                            None
                        } else {
                            Some(link_add)
                        }
                    }
                    None => None,
                };
                let link_add = match maybe_link_add {
                    Some(link_add) => link_add,
                    // Handle link add missing
                    // Probably just waiting on StoreElement or RegisterAddLink
                    // to arrive so put back in queue with a log message
                    None => {
                        // Add op back on queue
                        workspace.integration_queue.put(
                            (Timestamp::now(), op_hash).try_into()?,
                            IntegrationQueueValue {
                                validation_status,
                                op,
                            },
                        )?;
                        continue;
                    }
                };

                // Remove the link
                workspace.meta.remove_link(
                    link_remove,
                    &link_add.base_address,
                    link_add.zome_id,
                    link_add.tag,
                )?;
            }
        }

        // TODO: PERF: Aviod this clone by returning the op on error
        let (op, basis) = match dht_op_to_light_basis(op.clone(), &workspace.cas).await {
            Ok(l) => l,
            Err(DhtOpConvertError::MissingHeaderEntry(_)) => {
                workspace.integration_queue.put(
                    (Timestamp::now(), op_hash).try_into()?,
                    IntegrationQueueValue {
                        validation_status,
                        op,
                    },
                )?;
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let value = IntegratedDhtOpsValue {
            validation_status,
            basis,
            op,
        };
        debug!(msg = "writing", ?op_hash);
        workspace.integrated_dht_ops.put(op_hash, value)?;
    }

    debug!("complete");
    Ok(WorkComplete::Complete)
}

pub struct IntegrateDhtOpsWorkspace<'env> {
    // integration queue
    integration_queue: IntegrationQueueStore<'env>,
    // integrated ops
    integrated_dht_ops: IntegratedDhtOpsStore<'env>,
    // Cas for storing
    cas: ChainCasBuf<'env>,
    // metadata store
    meta: MetadataBuf<'env>,
}

impl<'env> Workspace<'env> for IntegrateDhtOpsWorkspace<'env> {
    /// Constructor
    fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBuf::new(reader, db)?;

        let db = dbs.get_db(&*INTEGRATION_QUEUE)?;
        let integration_queue = KvBuf::new(reader, db)?;

        let cas = ChainCasBuf::primary(reader, dbs, true)?;
        let meta = MetadataBuf::primary(reader, dbs)?;

        Ok(Self {
            integration_queue,
            integrated_dht_ops,
            cas,
            meta,
        })
    }
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        // flush cas
        self.cas.flush_to_txn(writer)?;
        // flush metadata store
        self.meta.flush_to_txn(writer)?;
        // flush integrated
        self.integrated_dht_ops.flush_to_txn(writer)?;
        // flush integration queue
        self.integration_queue.flush_to_txn(writer)?;
        Ok(())
    }
}
