//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::{
        dht_op_integration::{
            IntegratedDhtOpsStore, IntegratedDhtOpsValue, IntegrationLimboStore,
            IntegrationLimboValue,
        },
        element_buf::ElementBuf,
        metadata::{MetadataBuf, MetadataBufT},
        workspace::{Workspace, WorkspaceResult},
    },
};
use error::WorkflowResult;
use fallible_iterator::FallibleIterator;
use holo_hash::{DhtOpHash, HeaderHash};
use holochain_keystore::Signature;
use holochain_state::{
    buffer::BufferedStore,
    buffer::KvBufFresh,
    db::{INTEGRATED_DHT_OPS, INTEGRATION_LIMBO},
    error::DatabaseResult,
    fresh_reader,
    prelude::*,
};
use holochain_types::{
    dht_op::{produce_op_lights_from_elements, DhtOp, DhtOpLight},
    element::{Element, SignedHeaderHashed, SignedHeaderHashedExt},
    validate::ValidationStatus,
    Entry, EntryHashed, Timestamp,
};
use holochain_zome_types::{element::SignedHeader, Header};
use produce_dht_ops_workflow::dht_op_light::{
    error::{DhtOpConvertError, DhtOpConvertResult},
    light_to_op,
};
use std::{collections::BinaryHeap, convert::TryInto};
use sys_validation_workflow::types::{DhtOpOrder, OrderedOp};
use tracing::*;

pub use disintegrate::*;

mod disintegrate;
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
        let op = light_to_op(iv.op.clone(), &workspace.element_judged).await?;
        let hash = DhtOpHash::with_data_sync(&op);
        let order = DhtOpOrder::from(&op);
        let v = OrderedOp {
            order,
            hash,
            op,
            value: iv,
        };
        sorted_ops.push(std::cmp::Reverse(v));
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
        for so in sorted_ops {
            let OrderedOp {
                hash,
                op,
                value,
                order,
            } = so.0;
            // Check validation status and put in correct dbs
            let outcome = match value.validation_status {
                ValidationStatus::Valid => integrate_single_dht_op(
                    value.clone(),
                    op,
                    &mut workspace.elements,
                    &mut workspace.meta,
                )?,
                ValidationStatus::Rejected => integrate_single_dht_op(
                    value.clone(),
                    op,
                    &mut workspace.element_rejected,
                    &mut workspace.meta_rejected,
                )?,
                ValidationStatus::Abandoned => {
                    // Throwing away abandoned ops
                    // TODO: keep abandoned ops but remove the entries
                    // and put them in a AbandonedPrefix db
                    continue;
                }
            };
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
                Outcome::Deferred(op) => next_ops.push(std::cmp::Reverse(OrderedOp {
                    hash,
                    order,
                    op,
                    value,
                })),
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
            let so = so.0;
            // TODO: it may be desirable to retain the original timestamp
            // when re-adding items to the queue for later processing. This is
            // challenging for now since we don't have access to that original
            // key. Just a possible note for the future.
            workspace.integration_limbo.put(so.hash, so.value)?;
        }
        WorkComplete::Incomplete
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

/// Integrate a single DhtOp to the specified stores.
///
/// The two stores are intended to be either the pair of Vaults,
/// or the pair of Caches, but never a mixture of the two.
///
/// We can skip integrating element data when integrating data as an Author
/// rather than as an Authority, hence the last parameter.
#[instrument(skip(iv, element_store, meta_store))]
fn integrate_single_dht_op<P: PrefixType>(
    iv: IntegrationLimboValue,
    op: DhtOp,
    element_store: &mut ElementBuf<P>,
    meta_store: &mut MetadataBuf<P>,
) -> DhtOpConvertResult<Outcome> {
    if op_dependencies_held(&op, element_store)? {
        integrate_single_data(op, element_store)?;
        integrate_single_metadata(iv.op.clone(), element_store, meta_store)?;
        let integrated = IntegratedDhtOpsValue {
            validation_status: iv.validation_status,
            op: iv.op,
            when_integrated: Timestamp::now(),
        };
        debug!("integrating");
        Ok(Outcome::Integrated(integrated))
    } else {
        debug!("deferring");
        Ok(Outcome::Deferred(op))
    }
}

/// Check if we have the required dependencies held before integrating.
// TODO: This doesn't really check why we are holding the values.
// We could have them for other reasons.
fn op_dependencies_held<P: PrefixType>(
    op: &DhtOp,
    element_store: &ElementBuf<P>,
) -> DhtOpConvertResult<bool> {
    {
        fn header_with_entry_is_stored<P: PrefixType>(
            hash: &HeaderHash,
            element_store: &ElementBuf<P>,
        ) -> DhtOpConvertResult<bool> {
            match element_store.get_header(hash)?.map(|e| {
                e.header()
                    .entry_data()
                    .map(|(h, _)| h.clone())
                    .ok_or_else(|| {
                        // This is not a NewEntryHeader: cannot continue
                        DhtOpConvertError::MissingEntryDataForHeader(hash.clone())
                    })
            }) {
                Some(r) => Ok(element_store.contains_entry(&r?)?),
                None => Ok(false),
            }
        }

        // let entry_is_stored = |hash| element_store.contains_entry(hash);

        let header_is_stored = |hash| element_store.contains_header(hash);

        match op {
            DhtOp::StoreElement(_, _, _)
            | DhtOp::StoreEntry(_, _, _)
            | DhtOp::RegisterAgentActivity(_, _) => (),
            DhtOp::RegisterUpdatedBy(_, entry_update) => {
                // Check if we have the header with entry that we are updating in the vault
                // or defer the op.
                if !header_with_entry_is_stored(
                    &entry_update.original_header_address,
                    element_store,
                )? {
                    return Ok(false);
                }
            }
            DhtOp::RegisterDeletedEntryHeader(_, element_delete) => {
                // Check if we have the header with the entry that we are removing in the vault
                // or defer the op.
                if !header_with_entry_is_stored(&element_delete.deletes_address, element_store)? {
                    return Ok(false);
                }
            }
            DhtOp::RegisterDeletedBy(_, element_delete) => {
                // Check if we have the header with the entry that we are removing in the vault
                // or defer the op.
                if !header_with_entry_is_stored(&element_delete.deletes_address, element_store)? {
                    return Ok(false);
                }
            }
            DhtOp::RegisterAddLink(_signature, _link_add) => {
                // TODO: Not sure what to do here as the base might be rejected
                // // Check whether we have the base address in the Vault.
                // // If not then this should put the op back on the queue.
                // if !entry_is_stored(&link_add.base_address)? {
                //     let op = DhtOp::RegisterAddLink(signature, link_add);
                //     return Outcome::deferred(op);
                // }
            }
            DhtOp::RegisterRemoveLink(_signature, link_remove) => {
                // TODO: Not sure what to do here as the base might be rejected
                // // Check whether we have the base address and link add address
                // // are in the Vault.
                // // If not then this should put the op back on the queue.
                // if !entry_is_stored(&link_remove.base_address)?
                //     || !header_is_stored(&link_remove.link_add_address)?
                // {
                //      return false;
                // }
                if !header_is_stored(&link_remove.link_add_address)? {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }
}

pub fn integrate_single_metadata<C, P>(
    op: DhtOpLight,
    element_store: &ElementBuf<P>,
    meta_store: &mut C,
) -> DhtOpConvertResult<()>
where
    P: PrefixType,
    C: MetadataBufT<P>,
{
    match op {
        DhtOpLight::StoreElement(hash, _, _) => {
            let header = get_header(hash, element_store)?;
            meta_store.register_element_header(&header)?;
        }
        DhtOpLight::StoreEntry(hash, _, _) => {
            let new_entry_header = get_header(hash, element_store)?.try_into()?;
            // Reference to headers
            meta_store.register_header(new_entry_header)?;
        }
        DhtOpLight::RegisterAgentActivity(hash, _) => {
            let header = get_header(hash, element_store)?;
            // register agent activity on this agents pub key
            meta_store.register_activity(header)?;
        }
        DhtOpLight::RegisterUpdatedBy(hash, _, _) => {
            let header = get_header(hash, element_store)?.try_into()?;
            meta_store.register_update(header)?;
        }
        DhtOpLight::RegisterDeletedEntryHeader(hash, _)
        | DhtOpLight::RegisterDeletedBy(hash, _) => {
            let header = get_header(hash, element_store)?.try_into()?;
            meta_store.register_delete(header)?
        }
        DhtOpLight::RegisterAddLink(hash, _) => {
            let header = get_header(hash, element_store)?.try_into()?;
            meta_store.add_link(header)?;
        }
        DhtOpLight::RegisterRemoveLink(hash, _) => {
            let header = get_header(hash, element_store)?.try_into()?;
            meta_store.delete_link(header)?;
        }
    }
    Ok(())
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
            DhtOp::RegisterUpdatedBy(signature, entry_update) => {
                put_data(signature, entry_update.into(), None, element_store)?;
            }
            DhtOp::RegisterDeletedEntryHeader(signature, element_delete) => {
                put_data(signature, element_delete.into(), None, element_store)?;
            }
            DhtOp::RegisterDeletedBy(signature, element_delete) => {
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

fn get_header<P: PrefixType>(
    hash: HeaderHash,
    element_store: &ElementBuf<P>,
) -> DhtOpConvertResult<Header> {
    Ok(element_store
        .get_header(&hash)?
        .ok_or_else(|| DhtOpConvertError::MissingData(hash.into()))?
        .into_header_and_signature()
        .0
        .into_content())
}

/// After writing an Element to our chain, we want to integrate the meta ops
/// inline, so that they are immediately available in the meta cache.
/// NB: We skip integrating the element data, since it is already available in
/// our vault.
pub async fn integrate_to_cache<C: MetadataBufT>(
    element: &Element,
    element_store: &ElementBuf,
    meta_store: &mut C,
) -> DhtOpConvertResult<()> {
    // Produce the light directly
    for op in produce_op_lights_from_elements(vec![element]).await? {
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
    // integration queue
    pub integration_limbo: IntegrationLimboStore,
    // integrated ops
    pub integrated_dht_ops: IntegratedDhtOpsStore,
    // Cas for storing
    pub elements: ElementBuf,
    // metadata store
    pub meta: MetadataBuf,
    // Data that has progressed past validation and is pending Integration
    pub element_judged: ElementBuf<JudgedPrefix>,
    pub meta_judged: MetadataBuf<JudgedPrefix>,
    pub element_rejected: ElementBuf<RejectedPrefix>,
    pub meta_rejected: MetadataBuf<RejectedPrefix>,
    // Ops to disintegrate
    pub to_disintegrate_judged: Vec<DhtOpLight>,
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
        self.element_judged.flush_to_txn_ref(writer)?;
        self.meta_judged.flush_to_txn_ref(writer)?;
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

        let elements = ElementBuf::vault(env.clone(), true)?;
        let meta = MetadataBuf::vault(env.clone())?;

        let element_judged = ElementBuf::judged(env.clone())?;
        let meta_judged = MetadataBuf::judged(env.clone())?;

        let element_rejected = ElementBuf::rejected(env.clone())?;
        let meta_rejected = MetadataBuf::rejected(env)?;

        Ok(Self {
            integration_limbo,
            integrated_dht_ops,
            elements,
            meta,
            element_judged,
            meta_judged,
            element_rejected,
            meta_rejected,
            to_disintegrate_judged: Vec::new(),
        })
    }

    #[tracing::instrument(skip(self, hash))]
    fn integrate(&mut self, hash: DhtOpHash, v: IntegratedDhtOpsValue) -> DhtOpConvertResult<()> {
        disintegrate_single_metadata(v.op.clone(), &self.element_judged, &mut self.meta_judged)?;
        self.to_disintegrate_judged.push(v.op.clone());
        self.integrated_dht_ops.put(hash, v)?;
        Ok(())
    }

    pub fn op_exists(&self, hash: &DhtOpHash) -> DatabaseResult<bool> {
        Ok(self.integrated_dht_ops.contains(&hash)? || self.integration_limbo.contains(&hash)?)
    }

    #[tracing::instrument(skip(self, writer))]
    /// We need to cancel any deletes for the judged data
    /// where the ops still in integration limbo reference that data
    fn update_element_stores(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        for op in self.to_disintegrate_judged.drain(..) {
            disintegrate_single_data(op, &mut self.element_judged);
        }
        let mut val_iter = self.integration_limbo.iter(writer)?;
        while let Some((_, vlv)) = val_iter.next()? {
            reintegrate_single_data(vlv.op, &mut self.element_judged);
        }
        Ok(())
    }
}
