//! The workflow and queue consumer for DhtOp integration

use super::incoming_dht_ops_workflow::IncomingDhtOpsWorkspace;
use super::*;
use crate::core::queue_consumer::OneshotWriter;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::validation::DhtOpOrder;
use crate::core::validation::OrderedOp;
use error::WorkflowResult;
use fallible_iterator::FallibleIterator;
use holo_hash::DhtOpHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_cascade::error::CascadeResult;
use holochain_cascade::Cascade;
use holochain_cascade::DbPair;
use holochain_cascade::{error::CascadeError, integrate_single_metadata};
use holochain_conductor_api::IntegrationStateDump;
use holochain_lmdb::buffer::BufferedStore;
use holochain_lmdb::buffer::KvBufFresh;
use holochain_lmdb::db::INTEGRATED_DHT_OPS;
use holochain_lmdb::db::INTEGRATION_LIMBO;
use holochain_lmdb::error::DatabaseResult;
use holochain_lmdb::fresh_reader;
use holochain_lmdb::prelude::*;
use holochain_state::prelude::*;
use holochain_types::prelude::*;

use holochain_zome_types::Entry;
use holochain_zome_types::ValidationStatus;

use produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertResult;
use produce_dht_ops_workflow::dht_op_light::light_to_op;
use std::collections::BinaryHeap;
use std::convert::TryInto;
use tracing::*;

pub use disintegrate::*;

mod disintegrate;

#[cfg(feature = "test_utils")]
mod tests;

#[instrument(skip(workspace, writer, trigger_sys))]
pub async fn integrate_dht_ops_workflow(
    mut workspace: IntegrateDhtOpsWorkspace,
    writer: OneshotWriter,
    trigger_sys: &mut TriggerSender,
) -> WorkflowResult<WorkComplete> {
    // one of many possible ways to access the env
    let env = workspace.elements.headers().env().clone();
    // Pull ops out of queue
    // TODO: PERF: Combine this collect with the sort when ElementBuf gets
    // aren't async
    let ops: Vec<IntegrationLimboValue> = fresh_reader!(env, |r| workspace
        .integration_limbo
        .drain_iter(&r)?
        .collect())?;

    // Sort the ops
    let mut sorted_ops = BinaryHeap::new();
    for iv in ops {
        let op = light_to_op(iv.op.clone(), &workspace.element_pending)?;
        let hash = DhtOpHash::with_data_sync(&op);
        let order = DhtOpOrder::from(&op);
        let v = OrderedOp {
            order,
            hash,
            op,
            value: iv,
        };
        sorted_ops.push(v);
    }

    let mut total_integrated: usize = 0;

    // Try to process the queue over and over again, until we either exhaust
    // the queue, or we can no longer integrate anything in the queue.
    // We do this because items in the queue may depend on one another but may
    // be out-of-order wrt. dependencies, so there is a chance that by repeating
    // integration, we may be able to integrate at least one more item.
    loop {
        let mut num_integrated: usize = 0;
        let mut next_ops = BinaryHeap::new();
        for so in sorted_ops.into_sorted_vec() {
            let OrderedOp {
                hash,
                op,
                value,
                order,
            } = so;
            // Check validation status and put in correct dbs
            let outcome = integrate_single_dht_op(value.clone(), op, &mut workspace).await?;
            match outcome {
                Outcome::Integrated(integrated) => {
                    // TODO We could create a prefix for the integrated ops db
                    // and separate rejected ops from valid ops.
                    // Currently you need to check the IntegratedDhtOpsValue for
                    // the status
                    workspace.integrate(hash, integrated)?;
                    num_integrated += 1;
                    total_integrated += 1;
                }
                Outcome::Deferred(op) => next_ops.push(OrderedOp {
                    hash,
                    order,
                    op,
                    value,
                }),
            }
        }
        sorted_ops = next_ops;
        // Either all ops are integrated or we couldn't integrate any on this pass
        if sorted_ops.is_empty() || num_integrated == 0 {
            break;
        }
    }

    let result = if sorted_ops.is_empty() {
        // There were no ops deferred, meaning we exhausted the queue
        WorkComplete::Complete
    } else {
        // Re-add the remaining ops to the queue, to be picked up next time.
        for so in sorted_ops {
            // TODO: it may be desirable to retain the original timestamp
            // when re-adding items to the queue for later processing. This is
            // challenging for now since we don't have access to that original
            // key. Just a possible note for the future.
            workspace.integration_limbo.put(so.hash, so.value)?;
        }
        WorkComplete::Complete
    };

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer.with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))?;

    // trigger other workflows

    if total_integrated > 0 {
        trigger_sys.trigger();
    }

    Ok(result)
}

/// Integrate a single DhtOp to the stores based on the
/// validation status.
///
/// Check for dependencies in any of our other stores.
#[instrument(skip(iv, workspace))]
async fn integrate_single_dht_op(
    iv: IntegrationLimboValue,
    op: DhtOp,
    workspace: &mut IntegrateDhtOpsWorkspace,
) -> WorkflowResult<Outcome> {
    if op_dependencies_held(&op, workspace).await? {
        match iv.validation_status {
            ValidationStatus::Valid => Ok(integrate_data_and_meta(
                iv,
                op,
                &mut workspace.elements,
                &mut workspace.meta,
            )?),
            ValidationStatus::Rejected => {
                update_activity_status(&op, &mut workspace.meta)?;
                update_validation_status(&op, &mut workspace.meta)?;
                Ok(integrate_data(iv, op, &mut workspace.element_rejected)?)
            }
            ValidationStatus::Abandoned => {
                // Throwing away abandoned ops
                // TODO: keep abandoned ops but remove the entries
                // and put them in a AbandonedPrefix db
                let integrated = IntegratedDhtOpsValue {
                    validation_status: iv.validation_status,
                    op: iv.op,
                    when_integrated: timestamp::now(),
                };
                Ok(Outcome::Integrated(integrated))
            }
        }
    } else {
        debug!("deferring");
        Ok(Outcome::Deferred(op))
    }
}

fn integrate_data_and_meta<P: PrefixType>(
    iv: IntegrationLimboValue,
    op: DhtOp,
    element_store: &mut ElementBuf<P>,
    meta_store: &mut MetadataBuf<P>,
) -> DhtOpConvertResult<Outcome> {
    integrate_single_data(op, element_store)?;
    integrate_single_metadata(iv.op.clone(), element_store, meta_store)?;
    let integrated = IntegratedDhtOpsValue {
        validation_status: iv.validation_status,
        op: iv.op,
        when_integrated: timestamp::now(),
    };
    debug!("integrating");
    Ok(Outcome::Integrated(integrated))
}

/// Integrate data only
fn integrate_data<P: PrefixType>(
    iv: IntegrationLimboValue,
    op: DhtOp,
    element_store: &mut ElementBuf<P>,
) -> DhtOpConvertResult<Outcome> {
    integrate_single_data(op, element_store)?;
    let integrated = IntegratedDhtOpsValue {
        validation_status: iv.validation_status,
        op: iv.op,
        when_integrated: timestamp::now(),
    };
    debug!("integrating");
    Ok(Outcome::Integrated(integrated))
}

/// Update the status of agent activity if an op
/// is rejected by the agent authority.
fn update_activity_status(
    op: &DhtOp,
    meta_integrated: &mut impl MetadataBufT,
) -> WorkflowResult<()> {
    if let DhtOp::RegisterAgentActivity(_, h) = &op {
        let chain_head = ChainHead {
            header_seq: h.header_seq(),
            hash: HeaderHash::with_data_sync(h),
        };
        meta_integrated.register_activity_status(h.author(), ChainStatus::Invalid(chain_head))?;
        meta_integrated.register_activity(h, ValidationStatus::Rejected)?;
    }
    Ok(())
}

/// Rejected headers still need to be stored in the metadata vault so
/// they can be served for a get details call.
fn update_validation_status(
    op: &DhtOp,
    meta_integrated: &mut impl MetadataBufT,
) -> WorkflowResult<()> {
    match op {
        DhtOp::StoreElement(_, h, _) => meta_integrated.register_rejected_element_header(h)?,
        DhtOp::StoreEntry(_, h, _) => meta_integrated.register_rejected_header(h.clone())?,
        _ => {}
    }
    Ok(())
}

/// Check if we have the required dependencies held before integrating.
async fn op_dependencies_held(
    op: &DhtOp,
    workspace: &mut IntegrateDhtOpsWorkspace,
) -> CascadeResult<bool> {
    {
        match op {
            DhtOp::StoreElement(_, _, _) | DhtOp::StoreEntry(_, _, _) => {}
            DhtOp::RegisterAgentActivity(_, header) => {
                // RegisterAgentActivity is the exception where we need to make
                // sure that we have integrated the previous RegisterAgentActivity DhtOp
                // from the same chain.
                // This is because to say if the chain is valid as a whole we can't
                // have parts missing.
                let mut cascade = workspace.cascade();
                let prev_header_hash = header.prev_header();
                if let Some(prev_header_hash) = prev_header_hash.cloned() {
                    match cascade
                        .retrieve_header(prev_header_hash, Default::default())
                        .await?
                    {
                        Some(prev_header) => {
                            let op_hash = DhtOpHash::with_data_sync(
                                &UniqueForm::RegisterAgentActivity(prev_header.header()),
                            );
                            if workspace.integration_limbo.contains(&op_hash)? {
                                return Ok(true);
                            }
                        }
                        None => return Ok(false),
                    }
                }
            }
            DhtOp::RegisterUpdatedContent(_, entry_update, _)
            | DhtOp::RegisterUpdatedElement(_, entry_update, _) => {
                // Check if we have the header with entry that we are updating
                // or defer the op.
                if !header_with_entry_is_stored(
                    &entry_update.original_header_address,
                    workspace.cascade(),
                )
                .await?
                {
                    return Ok(false);
                }
            }
            DhtOp::RegisterDeletedBy(_, element_delete)
            | DhtOp::RegisterDeletedEntryHeader(_, element_delete) => {
                // Check if we have the header with the entry that we are removing
                // or defer the op.
                if !header_with_entry_is_stored(
                    &element_delete.deletes_address,
                    workspace.cascade(),
                )
                .await?
                {
                    return Ok(false);
                }
            }
            DhtOp::RegisterAddLink(_, create_link) => {
                // Check whether we have the base address.
                // If not then this should put the op back on the queue.
                if !entry_is_stored(&create_link.base_address, workspace.cascade()).await? {
                    return Ok(false);
                }
            }
            DhtOp::RegisterRemoveLink(_, delete_link) => {
                // Check whether we have the link add address.
                // If not then this should put the op back on the queue.
                if !header_is_stored(&delete_link.link_add_address, workspace.cascade()).await? {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }
}

/// Check the cascade to see if this Header is also stored with an Entry
async fn header_with_entry_is_stored(
    hash: &HeaderHash,
    mut cascade: Cascade<'_>,
) -> CascadeResult<bool> {
    // TODO: PERF: Add contains() to cascade so we don't deserialize
    // the entry
    match cascade
        .retrieve(hash.clone().into(), Default::default())
        .await?
    {
        Some(el) => match el.entry() {
            ElementEntry::Present(_) | ElementEntry::Hidden => Ok(true),
            ElementEntry::NotApplicable => Err(CascadeError::EntryMissing(hash.clone())),
            // This means we have just the header (probably through register agent activity)
            ElementEntry::NotStored => Ok(false),
        },
        None => Ok(false),
    }
}

/// Check if an Entry is stored in the cascade
async fn entry_is_stored(hash: &EntryHash, mut cascade: Cascade<'_>) -> CascadeResult<bool> {
    // TODO: PERF: Add contains() to cascade so we don't deserialize
    // the entry
    match cascade
        .retrieve_entry(hash.clone(), Default::default())
        .await?
    {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

/// Check if a header is stored in the cascade
async fn header_is_stored(hash: &HeaderHash, mut cascade: Cascade<'_>) -> CascadeResult<bool> {
    match cascade
        .retrieve_header(hash.clone(), Default::default())
        .await?
    {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

/// Store a DhtOp's data in an element buf
pub fn integrate_single_data<P: PrefixType>(
    op: DhtOp,
    element_store: &mut ElementBuf<P>,
) -> DhtOpConvertResult<()> {
    {
        match op {
            DhtOp::StoreElement(signature, header, maybe_entry) => {
                put_data(signature, header, maybe_entry.map(|e| *e), element_store)?;
            }
            DhtOp::StoreEntry(signature, new_entry_header, entry) => {
                put_data(
                    signature,
                    new_entry_header.into(),
                    Some(*entry),
                    element_store,
                )?;
            }
            DhtOp::RegisterAgentActivity(signature, header) => {
                put_data(signature, header, None, element_store)?;
            }
            DhtOp::RegisterUpdatedContent(signature, entry_update, _)
            | DhtOp::RegisterUpdatedElement(signature, entry_update, _) => {
                put_data(signature, entry_update.into(), None, element_store)?;
            }
            DhtOp::RegisterDeletedEntryHeader(signature, element_delete)
            | DhtOp::RegisterDeletedBy(signature, element_delete) => {
                put_data(signature, element_delete.into(), None, element_store)?;
            }
            DhtOp::RegisterAddLink(signature, link_add) => {
                put_data(signature, link_add.into(), None, element_store)?;
            }
            DhtOp::RegisterRemoveLink(signature, link_remove) => {
                put_data(signature, link_remove.into(), None, element_store)?;
            }
        }
        Ok(())
    }
}

fn put_data<P: PrefixType>(
    signature: Signature,
    header: Header,
    maybe_entry: Option<Entry>,
    element_store: &mut ElementBuf<P>,
) -> DhtOpConvertResult<()> {
    let signed_header = SignedHeaderHashed::from_content_sync(SignedHeader(header, signature));
    let maybe_entry_hashed = match maybe_entry {
        Some(entry) => Some(EntryHashed::from_content_sync(entry)),
        None => None,
    };
    element_store.put(signed_header, maybe_entry_hashed)?;
    Ok(())
}

/// After writing an Element to our chain, we want to integrate the meta ops
/// inline, so that they are immediately available in the authored metadata.
/// NB: We skip integrating the element data, since it is already available in
/// our source chain.
pub fn integrate_to_authored<C: MetadataBufT<AuthoredPrefix>>(
    element: &Element,
    element_store: &ElementBuf<AuthoredPrefix>,
    meta_store: &mut C,
) -> DhtOpConvertResult<()> {
    // Produce the light directly
    for op in produce_op_lights_from_elements(vec![element])? {
        // we don't integrate element data, because it is already in our vault.
        integrate_single_metadata(op, element_store, meta_store)?
    }
    Ok(())
}

/// The outcome of integrating a single DhtOp: either it was, or it wasn't
enum Outcome {
    Integrated(IntegratedDhtOpsValue),
    Deferred(DhtOp),
}

pub struct IntegrateDhtOpsWorkspace {
    /// integration queue
    pub integration_limbo: IntegrationLimboStore,
    /// integrated ops
    pub integrated_dht_ops: IntegratedDhtOpsStore,
    /// Cas for storing
    pub elements: ElementBuf,
    /// metadata store
    pub meta: MetadataBuf,
    /// Data that has progressed past validation and is pending Integration
    pub element_pending: ElementBuf<PendingPrefix>,
    pub meta_pending: MetadataBuf<PendingPrefix>,
    pub element_rejected: ElementBuf<RejectedPrefix>,
    pub meta_rejected: MetadataBuf<RejectedPrefix>,
    /// Ops to disintegrate
    pub to_disintegrate_pending: Vec<DhtOpLight>,
    /// READ ONLY
    /// Need the validation limbo to make sure we don't
    /// remove data that is in this limbo
    pub validation_limbo: ValidationLimboStore,
}

impl Workspace for IntegrateDhtOpsWorkspace {
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.update_element_stores(writer)?;
        // flush elements
        self.elements.flush_to_txn_ref(writer)?;
        // flush metadata store
        self.meta.flush_to_txn_ref(writer)?;
        // flush integrated
        self.integrated_dht_ops.flush_to_txn_ref(writer)?;
        // flush integration queue
        self.integration_limbo.flush_to_txn_ref(writer)?;
        self.element_pending.flush_to_txn_ref(writer)?;
        self.meta_pending.flush_to_txn_ref(writer)?;
        self.element_rejected.flush_to_txn_ref(writer)?;
        self.meta_rejected.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

impl IntegrateDhtOpsWorkspace {
    /// Constructor
    pub fn new(env: EnvironmentRead) -> WorkspaceResult<Self> {
        let db = env.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBufFresh::new(env.clone(), db);

        let db = env.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBufFresh::new(env.clone(), db);

        let validation_limbo = ValidationLimboStore::new(env.clone())?;

        let elements = ElementBuf::vault(env.clone(), true)?;
        let meta = MetadataBuf::vault(env.clone())?;

        let element_pending = ElementBuf::pending(env.clone())?;
        let meta_pending = MetadataBuf::pending(env.clone())?;

        let element_rejected = ElementBuf::rejected(env.clone())?;
        let meta_rejected = MetadataBuf::rejected(env)?;

        Ok(Self {
            integration_limbo,
            integrated_dht_ops,
            elements,
            meta,
            element_pending,
            meta_pending,
            element_rejected,
            meta_rejected,
            validation_limbo,
            to_disintegrate_pending: Vec::new(),
        })
    }

    #[tracing::instrument(skip(self, hash))]
    fn integrate(&mut self, hash: DhtOpHash, v: IntegratedDhtOpsValue) -> DhtOpConvertResult<()> {
        disintegrate_single_metadata(v.op.clone(), &self.element_pending, &mut self.meta_pending)?;
        self.to_disintegrate_pending.push(v.op.clone());
        self.integrated_dht_ops.put(hash, v)?;
        Ok(())
    }

    pub fn op_exists(&self, hash: &DhtOpHash) -> DatabaseResult<bool> {
        Ok(self.integrated_dht_ops.contains(&hash)? || self.integration_limbo.contains(&hash)?)
    }

    /// Create a cascade through the integrated and rejected stores
    // TODO: Might need to add abandoned here but will need some
    // thought as abandoned entries are not stored.
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

    #[tracing::instrument(skip(self, writer))]
    /// We need to cancel any deletes for the judged data
    /// where the ops still in integration limbo reference that data
    fn update_element_stores(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        for op in self.to_disintegrate_pending.drain(..) {
            disintegrate_single_data(op, &mut self.element_pending);
        }
        let mut int_iter = self.integration_limbo.iter(writer)?;
        while let Some((_, vlv)) = int_iter.next()? {
            reintegrate_single_data(vlv.op, &mut self.element_pending);
        }
        let mut val_iter = self.validation_limbo.iter(writer)?;
        while let Some((_, vlv)) = val_iter.next()? {
            reintegrate_single_data(vlv.op, &mut self.element_pending);
        }
        Ok(())
    }
}

pub fn dump_state(env: EnvironmentRead) -> WorkspaceResult<IntegrationStateDump> {
    let workspace = IncomingDhtOpsWorkspace::new(env.clone())?;
    let (validation_limbo, integration_limbo, integrated) = fresh_reader!(env, |r| {
        let v = workspace.validation_limbo.iter(&r)?.count()?;
        let il = workspace.integration_limbo.iter(&r)?.count()?;
        let i = workspace.integrated_dht_ops.iter(&r)?.count()?;
        DatabaseResult::Ok((v, il, i))
    })?;

    Ok(IntegrationStateDump {
        validation_limbo,
        integration_limbo,
        integrated,
    })
}
