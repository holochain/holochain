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
        metadata::{MetadataBuf, MetadataBufT},
        workspace::{Workspace, WorkspaceResult},
    },
};
use error::WorkflowResult;
use fallible_iterator::FallibleIterator;
use holo_hash::HasHash;
use holochain_state::{
    buffer::BufferedStore,
    buffer::KvBuf,
    db::{INTEGRATED_DHT_OPS, INTEGRATION_QUEUE},
    prelude::{GetDb, Reader, Writer},
};
use holochain_types::{
    dht_op::{produce_ops_from_element, DhtOp, DhtOpHashed},
    element::{Element, SignedHeaderHashed},
    validate::ValidationStatus,
    EntryHashed, HeaderHashed, Timestamp, TimestampKey,
};
use holochain_zome_types::header::IntendedFor;
use produce_dht_ops_workflow::dht_op_light::{
    dht_op_to_light_basis,
    error::{DhtOpConvertError, DhtOpConvertResult},
};
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
                match integrate_single_dht_op(
                    value,
                    &mut workspace.cas,
                    &mut workspace.meta,
                    IntegrationContext::Authority,
                )
                .await?
                {
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

/// Integrate a single DhtOp to the specified stores.
///
/// The two stores are intended to be either the pair of Vaults,
/// or the pair of Caches, but never a mixture of the two.
///
/// We can skip integrating element data when integrating data as an Author
/// rather than as an Authority, hence the last parameter.
#[instrument(skip(value, element_store, meta_store))]
async fn integrate_single_dht_op(
    value: IntegrationQueueValue,
    element_store: &mut ChainCasBuf<'_>,
    meta_store: &mut MetadataBuf<'_>,
    context: IntegrationContext,
) -> DhtOpConvertResult<Outcome> {
    use IntegrationContext::Authority;
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
                if context == Authority {
                    let header = HeaderHashed::from_content(header).await;
                    let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                    let maybe_entry_hashed = match maybe_entry {
                        Some(entry) => Some(EntryHashed::from_content(*entry).await),
                        None => None,
                    };
                    // Store the entry
                    element_store.put(signed_header, maybe_entry_hashed)?;
                }
            }
            DhtOp::StoreEntry(signature, new_entry_header, entry) => {
                // Reference to headers
                meta_store.register_header(new_entry_header.clone()).await?;

                if context == Authority {
                    let header = HeaderHashed::from_content(new_entry_header.into()).await;
                    let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                    let entry = EntryHashed::from_content(*entry).await;
                    // Store Header and Entry
                    element_store.put(signed_header, Some(entry))?;
                }
            }
            DhtOp::RegisterAgentActivity(signature, header) => {
                if context == Authority {
                    // Store header
                    let header_hashed = HeaderHashed::from_content(header.clone()).await;
                    let signed_header =
                        SignedHeaderHashed::with_presigned(header_hashed, signature);
                    element_store.put(signed_header, None)?;
                }

                // register agent activity on this agents pub key
                meta_store.register_activity(header).await?;
            }
            DhtOp::RegisterReplacedBy(_, entry_update, _) => {
                let old_entry_hash = match entry_update.intended_for {
                    IntendedFor::Header => None,
                    IntendedFor::Entry => {
                        match element_store
                            .get_header(&entry_update.replaces_address)
                            .await?
                            // Handle missing old entry header. Same reason as below
                            .and_then(|e| e.header().entry_data().map(|(hash, _)| hash.clone()))
                        {
                            Some(hash) => Some(hash),
                            // Handle missing old Entry (Probably StoreEntry hasn't arrived been processed)
                            // This is put the op back in the integration queue to try again later
                            None => return Outcome::deferred(op, validation_status),
                        }
                    }
                };
                meta_store
                    .register_update(entry_update, old_entry_hash)
                    .await?;
            }
            DhtOp::RegisterDeletedEntryHeader(_, entry_delete)
            | DhtOp::RegisterDeletedBy(_, entry_delete) => {
                let entry_hash = match element_store
                    .get_header(&entry_delete.removes_address)
                    .await?
                    // Handle missing entry header. Same reason as below
                    .and_then(|e| e.header().entry_data().map(|(hash, _)| hash.clone()))
                {
                    Some(hash) => hash,
                    // TODO: VALIDATION: This could also be an invalid delete on a header without a delete
                    // Handle missing Entry (Probably StoreEntry hasn't arrived been processed)
                    // This is put the op back in the integration queue to try again later
                    None => return Outcome::deferred(op, validation_status),
                };
                meta_store.register_delete(entry_delete, entry_hash).await?
            }
            DhtOp::RegisterAddLink(signature, link_add) => {
                if context == Authority {
                    // Check whether we have the base address in the Vault.
                    // If not then this should put the op back on the queue.
                    if element_store
                        .get_entry(&link_add.base_address)
                        .await?
                        .is_none()
                    {
                        return Outcome::deferred(op, validation_status);
                    }

                    // Store add Header
                    let header = HeaderHashed::from_content(link_add.clone().into()).await;
                    debug!(link_add = ?header.as_hash());

                    let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                    element_store.put(signed_header, None)?;
                }

                meta_store.add_link(link_add).await?;
            }
            DhtOp::RegisterRemoveLink(signature, link_remove) => {
                if context == Authority {
                    // Check whether we have the base address in the Vault.
                    // If not then this should put the op back on the queue.
                    if element_store
                        .get_entry(&link_remove.base_address)
                        .await?
                        .is_none()
                    {
                        return Outcome::deferred(op, validation_status);
                    }

                    // Store link delete Header
                    let header = HeaderHashed::from_content(link_remove.clone().into()).await;
                    let signed_header = SignedHeaderHashed::with_presigned(header, signature);
                    element_store.put(signed_header, None)?;
                }

                // Remove the link
                meta_store.remove_link(link_remove).await?;
            }
        }

        // TODO: PERF: Avoid this clone by returning the op on error
        let (op, basis) = match dht_op_to_light_basis(op.clone(), element_store).await {
            Ok(l) => l,
            Err(DhtOpConvertError::MissingHeaderEntry(_)) => {
                return Outcome::deferred(op, validation_status)
            }
            Err(e) => return Err(e),
        };
        let value = IntegratedDhtOpsValue {
            validation_status,
            basis,
            op,
            when_integrated: Timestamp::now(),
        };
        debug!("integrating");
        Ok(Outcome::Integrated(value))
    }
}

#[derive(PartialEq, std::fmt::Debug)]
/// Specifies my role when integrating
enum IntegrationContext {
    /// I am integrating DhtOps which I authored
    Author,
    /// I am integrating DhtOps which were published to me as an authority
    Authority,
}

/// After writing an Element to our chain, we want to integrate the meta ops
/// inline, so that they are immediately available in the meta cache.
/// NB: We skip integrating the element data, since it is already available in
/// our vault.
pub async fn integrate_to_cache(
    element: &Element,
    element_store: &mut ChainCasBuf<'_>,
    meta_store: &mut MetadataBuf<'_>,
) -> DhtOpConvertResult<()> {
    for op in produce_ops_from_element(element)? {
        let value = IntegrationQueueValue {
            op,
            validation_status: ValidationStatus::Valid,
        };
        // we don't integrate element data, because it is already in our vault.
        match integrate_single_dht_op(value, element_store, meta_store, IntegrationContext::Author)
            .await?
        {
            Outcome::Integrated(_) => {}
            Outcome::Deferred(v) => {
                // FIXME: if inline integration is deferred for any reason, we
                // expect sys validation to fail. Since sys validation is not
                // implemented, we panic here instead. When sys validation
                // lands, make this a warning
                // TODO: make this a panic after @freesig's dht_get work is in (B-01478)
                error!("An inline-integrated DhtOp was deferred. We expect sys validation to fail: {:?}", v)
            }
        }
    }
    Ok(())
}

/// The outcome of integrating a single DhtOp: either it was, or it wasn't
enum Outcome {
    Integrated(IntegratedDhtOpsValue),
    Deferred(IntegrationQueueValue),
}

impl Outcome {
    fn deferred(op: DhtOp, validation_status: ValidationStatus) -> DhtOpConvertResult<Self> {
        Ok(Outcome::Deferred(IntegrationQueueValue {
            op,
            validation_status,
        }))
    }
}

pub struct IntegrateDhtOpsWorkspace<'env> {
    // integration queue
    pub integration_queue: IntegrationQueueStore<'env>,
    // integrated ops
    pub integrated_dht_ops: IntegratedDhtOpsStore<'env>,
    // Cas for storing
    pub cas: ChainCasBuf<'env>,
    // metadata store
    pub meta: MetadataBuf<'env>,
}

impl<'env> Workspace<'env> for IntegrateDhtOpsWorkspace<'env> {
    /// Constructor
    fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBuf::new(reader, db)?;

        let db = dbs.get_db(&*INTEGRATION_QUEUE)?;
        let integration_queue = KvBuf::new(reader, db)?;

        let cas = ChainCasBuf::vault(reader, dbs, true)?;
        let meta = MetadataBuf::vault(reader, dbs)?;

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
