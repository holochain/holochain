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
    validate::ValidationStatus,
    EntryHashed, Header, HeaderHashed, TimestampKey,
};
use produce_dht_ops_workflow::dht_op_light::{dht_op_to_light_basis, error::DhtOpConvertError};
use tracing::*;

mod tests;

pub async fn integrate_dht_ops_workflow(
    mut workspace: IntegrateDhtOpsWorkspace<'_>,
    writer: OneshotWriter,
    trigger_publish: &mut TriggerSender,
) -> WorkflowResult<WorkComplete> {
    // Pull ops out of queue
    // TODO: PERF: we collect() only because this iterator cannot cross awaits,
    // but is there a way to do this without collect()?
    let ops: Vec<_> = workspace
        .integration_queue
        .drain_iter()?
        .iterator()
        .collect();

    // Compute hashes for all dht_ops, include in tuples alongside db values
    let mut ops = futures::future::join_all(
        ops.into_iter()
            .map(|val| {
                val.map(|val| async move {
                    let IntegrationQueueValue {
                        op,
                        validation_status,
                    } = val;
                    let (op, op_hash) = DhtOpHashed::from_content(op).await.into_inner();
                    (
                        op_hash,
                        IntegrationQueueValue {
                            op,
                            validation_status,
                        },
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    )
    .await;

    let mut total_integrated: usize = 0;

    // Try to process the queue over and over again, until we either exhaust
    // the queue, or we can no longer integrate anything in the queue.
    // We do this because items in the queue may depend on one another but may
    // be out-of-order wrt. dependencies, so there is a chance that by repeating
    // integration, we may be able to integrate at least one more item.
    //
    // A less naive approach would be to intelligently reorder the queue to
    // guarantee that intra-queue dependencies are correctly ordered, but ain't
    // nobody got time for that right now! TODO: make time for this?
    while {
        let mut num_integrated: usize = 0;
        let mut next_ops = Vec::new();
        for (op_hash, value) in ops {
            // only integrate this op if it hasn't been integrated already!
            // TODO: test for this [ B-01894 ]
            if workspace.integrated_dht_ops.get(&op_hash)?.is_none() {
                match integrate_single_dht_op(&mut workspace, value).await? {
                    Outcome::Integrated(integrated) => {
                        workspace.integrated_dht_ops.put(op_hash, integrated)?;
                        num_integrated += 1;
                        total_integrated += 1;
                    }
                    Outcome::Deferred(deferred) => next_ops.push((op_hash, deferred)),
                }
            }
        }
        ops = next_ops;
        ops.len() > 0 && num_integrated > 0
    } { /* NB: this is actually a do-while loop! */ }

    let result = if ops.len() == 0 {
        // There were no ops deferred, meaning we exhausted the queue
        WorkComplete::Complete
    } else {
        // Re-add the remaining ops to the queue, to be picked up next time.
        for (op_hash, value) in ops {
            // TODO: it may be desirable to retain the original timestamp
            // when re-adding items to the queue for later processing. This is
            // challenging for now since we don't have access to that original
            // key. Just a possible note for the future.
            workspace
                .integration_queue
                .put((TimestampKey::now(), op_hash).into(), value)?;
        }
        WorkComplete::Incomplete
    };

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| workspace.flush_to_txn(writer).expect("TODO"))
        .await?;

    // trigger other workflows

    // Only trigger publish if we have done any work during this workflow,
    // to prevent endless cascades of publishing. Ideally, we shouldn't trigger
    // publish unless we have integrated something we've authored, but this is
    // a step in that direction.
    // TODO: only trigger if we have integrated ops that we have authored
    if total_integrated > 0 {
        trigger_publish.trigger();
    }

    Ok(result)
}

#[instrument(skip(workspace, value))]
async fn integrate_single_dht_op(
    workspace: &mut IntegrateDhtOpsWorkspace<'_>,
    value: IntegrationQueueValue,
) -> WorkflowResult<Outcome> {
    debug!("Starting integrate dht ops workflow");
    {
        // Process each op
        let IntegrationQueueValue {
            op,
            validation_status,
        } = value;

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
                workspace.meta.register_activity(header).await?;
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
                            None => return Outcome::deferred(op, validation_status),
                        }
                    }
                };
                workspace
                    .meta
                    .register_update(entry_update, old_entry_hash)
                    .await?;
            }
            DhtOp::RegisterDeletedEntryHeader(_, entry_delete)
            | DhtOp::RegisterDeletedBy(_, entry_delete) => {
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
                    None => return Outcome::deferred(op, validation_status),
                };
                workspace
                    .meta
                    .register_delete(entry_delete, entry_hash)
                    .await?
            }
            DhtOp::RegisterAddLink(signature, link_add) => {
                workspace.meta.add_link(link_add.clone()).await?;
                // Store add Header
                let header = HeaderHashed::with_data(link_add.into()).await?;
                debug!(link_add = ?header.as_hash());
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
                    return Outcome::deferred(op, validation_status);
                }

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
                    None => return Outcome::deferred(op, validation_status),
                };

                // Store link delete Header
                let header = HeaderHashed::with_data(link_remove.clone().into()).await?;
                let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                workspace.cas.put(signed_header, None)?;

                // Remove the link
                workspace.meta.remove_link(
                    link_remove,
                    &link_add.base_address,
                    link_add.zome_id,
                    link_add.tag,
                )?;
            }
        }

        // TODO: PERF: Avoid this clone by returning the op on error
        let (op, basis) = match dht_op_to_light_basis(op.clone(), &workspace.cas).await {
            Ok(l) => l,
            Err(DhtOpConvertError::MissingHeaderEntry(_)) => {
                return Outcome::deferred(op, validation_status)
            }
            Err(e) => return Err(e.into()),
        };
        let value = IntegratedDhtOpsValue {
            validation_status,
            basis,
            op,
        };
        debug!("integrating");
        Ok(Outcome::Integrated(value))
    }
}

/// The outcome of integrating a single DhtOp: either it was, or it wasn't
enum Outcome {
    Integrated(IntegratedDhtOpsValue),
    Deferred(IntegrationQueueValue),
}

impl Outcome {
    fn deferred(op: DhtOp, validation_status: ValidationStatus) -> WorkflowResult<Self> {
        Ok(Outcome::Deferred(IntegrationQueueValue {
            op,
            validation_status,
        }))
    }
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
