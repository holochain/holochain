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
        let hash = DhtOpHash::with_data(&op).await;
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
            // TODO: Check validation status and put in correct dbs
            let outcome = match value.validation_status {
                ValidationStatus::Valid => {
                    integrate_single_dht_op(
                        value.clone(),
                        op,
                        &mut workspace.elements,
                        &mut workspace.meta,
                    )
                    .await?
                }
                ValidationStatus::Rejected => {
                    integrate_single_dht_op(
                        value.clone(),
                        op,
                        &mut workspace.element_rejected,
                        &mut workspace.meta_rejected,
                    )
                    .await?
                }
                ValidationStatus::Abandoned => {
                    // Throwing away abandoned ops
                    continue;
                }
            };
            match outcome {
                Outcome::Integrated(integrated) => {
                    // TODO We could create a prefix for the integrated ops db
                    // and separate rejected ops from valid ops.
                    // Currently you need to check the IntegratedDhtOpsValue for
                    // the status
                    workspace.integrate(hash, integrated).await?;
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
        if sorted_ops.len() == 0 || num_integrated == 0 {
            break;
        }
    }

    let result = if sorted_ops.len() == 0 {
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
#[instrument(skip(value, element_store, meta_store))]
async fn integrate_single_dht_op<P: PrefixType>(
    value: IntegrationLimboValue,
    op: DhtOp,
    element_store: &mut ElementBuf<P>,
    meta_store: &mut MetadataBuf<P>,
) -> DhtOpConvertResult<Outcome> {
    match integrate_single_element(value, op, element_store).await? {
        Outcome::Integrated(v) => {
            integrate_single_metadata(v.op.clone(), element_store, meta_store).await?;
            debug!("integrating");
            Ok(Outcome::Integrated(v))
        }
        v @ Outcome::Deferred(_) => {
            debug!("deferring");
            Ok(v)
        }
    }
}

async fn integrate_single_element<P: PrefixType>(
    iv: IntegrationLimboValue,
    op: DhtOp,
    element_store: &mut ElementBuf<P>,
) -> DhtOpConvertResult<Outcome> {
    {
        async fn header_with_entry_is_stored<P: PrefixType>(
            hash: &HeaderHash,
            element_store: &ElementBuf<P>,
        ) -> DhtOpConvertResult<bool> {
            match element_store.get_header(hash).await?.map(|e| {
                e.header()
                    .entry_data()
                    .map(|(h, _)| h.clone())
                    .ok_or_else(|| {
                        // This is not a NewEntryHeader: cannot continue
                        DhtOpConvertError::MissingEntryDataForHeader(hash.clone())
                    })
            }) {
                Some(r) => Ok(element_store.contains_entry(&r?).await?),
                None => Ok(false),
            }
        }

        // let entry_is_stored = |hash| element_store.contains_entry(hash);

        let header_is_stored = |hash| element_store.contains_header(hash);

        match op {
            DhtOp::StoreElement(signature, header, maybe_entry) => {
                put_data(signature, header, maybe_entry.map(|e| *e), element_store).await?;
            }
            DhtOp::StoreEntry(signature, new_entry_header, entry) => {
                put_data(
                    signature,
                    new_entry_header.into(),
                    Some(*entry),
                    element_store,
                )
                .await?;
            }
            DhtOp::RegisterAgentActivity(signature, header) => {
                put_data(signature, header, None, element_store).await?;
            }
            DhtOp::RegisterUpdatedBy(signature, entry_update) => {
                // Check if we have the header with entry that we are updating in the vault
                // or defer the op.
                if !header_with_entry_is_stored(
                    &entry_update.original_header_address,
                    element_store,
                )
                .await?
                {
                    let op = DhtOp::RegisterUpdatedBy(signature, entry_update);
                    return Outcome::deferred(op);
                }
                put_data(signature, entry_update.into(), None, element_store).await?;
            }
            DhtOp::RegisterDeletedEntryHeader(signature, element_delete) => {
                // Check if we have the header with the entry that we are removing in the vault
                // or defer the op.
                if !header_with_entry_is_stored(&element_delete.removes_address, element_store)
                    .await?
                {
                    // Can't combine the two delete match arms without cloning the op
                    let op = DhtOp::RegisterDeletedEntryHeader(signature, element_delete);
                    return Outcome::deferred(op);
                }
                put_data(signature, element_delete.into(), None, element_store).await?;
            }
            DhtOp::RegisterDeletedBy(signature, element_delete) => {
                // Check if we have the header with the entry that we are removing in the vault
                // or defer the op.
                if !header_with_entry_is_stored(&element_delete.removes_address, element_store)
                    .await?
                {
                    let op = DhtOp::RegisterDeletedBy(signature, element_delete);
                    return Outcome::deferred(op);
                }
                put_data(signature, element_delete.into(), None, element_store).await?;
            }
            DhtOp::RegisterAddLink(signature, link_add) => {
                // TODO: Not sure what to do here as the base might be rejected
                // // Check whether we have the base address in the Vault.
                // // If not then this should put the op back on the queue.
                // if !entry_is_stored(&link_add.base_address).await? {
                //     let op = DhtOp::RegisterAddLink(signature, link_add);
                //     return Outcome::deferred(op);
                // }

                put_data(signature, link_add.into(), None, element_store).await?;
            }
            DhtOp::RegisterRemoveLink(signature, link_remove) => {
                // TODO: Not sure what to do here as the base might be rejected
                // // Check whether we have the base address and link add address
                // // are in the Vault.
                // // If not then this should put the op back on the queue.
                // if !entry_is_stored(&link_remove.base_address).await?
                //     || !header_is_stored(&link_remove.link_add_address).await?
                // {
                //     let op = DhtOp::RegisterRemoveLink(signature, link_remove);
                //     return Outcome::deferred(op);
                // }
                if !header_is_stored(&link_remove.link_add_address).await? {
                    let op = DhtOp::RegisterRemoveLink(signature, link_remove);
                    return Outcome::deferred(op);
                }

                put_data(signature, link_remove.into(), None, element_store).await?;
            }
        }

        let value = IntegratedDhtOpsValue {
            validation_status: iv.validation_status,
            op: iv.op,
            when_integrated: Timestamp::now(),
        };
        Ok(Outcome::Integrated(value))
    }
}

pub async fn integrate_single_metadata<C, P>(
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
            let header = get_header(hash, element_store).await?;
            meta_store.register_element_header(&header).await?;
        }
        DhtOpLight::StoreEntry(hash, _, _) => {
            let new_entry_header = get_header(hash, element_store).await?.try_into()?;
            // Reference to headers
            meta_store.register_header(new_entry_header).await?;
        }
        DhtOpLight::RegisterAgentActivity(hash, _) => {
            let header = get_header(hash, element_store).await?;
            // register agent activity on this agents pub key
            meta_store.register_activity(header).await?;
        }
        DhtOpLight::RegisterUpdatedBy(hash, _, _) => {
            let header = get_header(hash, element_store).await?.try_into()?;
            meta_store.register_update(header).await?;
        }
        DhtOpLight::RegisterDeletedEntryHeader(hash, _)
        | DhtOpLight::RegisterDeletedBy(hash, _) => {
            let header = get_header(hash, element_store).await?.try_into()?;
            meta_store.register_delete(header).await?
        }
        DhtOpLight::RegisterAddLink(hash, _) => {
            let header = get_header(hash, element_store).await?.try_into()?;
            meta_store.add_link(header).await?;
        }
        DhtOpLight::RegisterRemoveLink(hash, _) => {
            let header = get_header(hash, element_store).await?.try_into()?;
            meta_store.remove_link(header).await?;
        }
    }
    debug!("made it");
    Ok(())
}

/// Store a DhtOp's data in an element buf without dependency checks
pub async fn integrate_single_op<P: PrefixType>(
    op: DhtOp,
    element_store: &mut ElementBuf<P>,
) -> DhtOpConvertResult<()> {
    {
        match op {
            DhtOp::StoreElement(signature, header, maybe_entry) => {
                put_data(signature, header, maybe_entry.map(|e| *e), element_store).await?;
            }
            DhtOp::StoreEntry(signature, new_entry_header, entry) => {
                put_data(
                    signature,
                    new_entry_header.into(),
                    Some(*entry),
                    element_store,
                )
                .await?;
            }
            DhtOp::RegisterAgentActivity(signature, header) => {
                put_data(signature, header, None, element_store).await?;
            }
            DhtOp::RegisterUpdatedBy(signature, entry_update) => {
                put_data(signature, entry_update.into(), None, element_store).await?;
            }
            DhtOp::RegisterDeletedEntryHeader(signature, element_delete) => {
                put_data(signature, element_delete.into(), None, element_store).await?;
            }
            DhtOp::RegisterDeletedBy(signature, element_delete) => {
                put_data(signature, element_delete.into(), None, element_store).await?;
            }
            DhtOp::RegisterAddLink(signature, link_add) => {
                put_data(signature, link_add.into(), None, element_store).await?;
            }
            DhtOp::RegisterRemoveLink(signature, link_remove) => {
                put_data(signature, link_remove.into(), None, element_store).await?;
            }
        }
        Ok(())
    }
}

async fn put_data<P: PrefixType>(
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

async fn get_header<P: PrefixType>(
    hash: HeaderHash,
    element_store: &ElementBuf<P>,
) -> DhtOpConvertResult<Header> {
    Ok(element_store
        .get_header(&hash)
        .await?
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
        integrate_single_metadata(op, element_store, meta_store).await?
    }
    Ok(())
}

/// The outcome of integrating a single DhtOp: either it was, or it wasn't
enum Outcome {
    Integrated(IntegratedDhtOpsValue),
    Deferred(DhtOp),
}

impl Outcome {
    fn deferred(op: DhtOp) -> DhtOpConvertResult<Self> {
        Ok(Outcome::Deferred(op))
    }
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
}

impl Workspace for IntegrateDhtOpsWorkspace {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        // flush elements
        self.elements.flush_to_txn(writer)?;
        // flush metadata store
        self.meta.flush_to_txn(writer)?;
        // flush integrated
        self.integrated_dht_ops.flush_to_txn(writer)?;
        // flush integration queue
        self.integration_limbo.flush_to_txn(writer)?;
        self.element_judged.flush_to_txn(writer)?;
        self.meta_judged.flush_to_txn(writer)?;
        self.element_rejected.flush_to_txn(writer)?;
        self.meta_rejected.flush_to_txn(writer)?;
        Ok(())
    }
}

impl IntegrateDhtOpsWorkspace {
    /// Constructor
    pub fn new(env: EnvironmentRead, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBufFresh::new(env.clone(), db);

        let db = dbs.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBufFresh::new(env.clone(), db);

        let elements = ElementBuf::vault(env.clone(), dbs, true)?;
        let meta = MetadataBuf::vault(env.clone(), dbs)?;

        let element_judged = ElementBuf::judged(env.clone(), dbs)?;
        let meta_judged = MetadataBuf::judged(env.clone(), dbs)?;

        let element_rejected = ElementBuf::rejected(env.clone(), dbs)?;
        let meta_rejected = MetadataBuf::rejected(env, dbs)?;

        Ok(Self {
            integration_limbo,
            integrated_dht_ops,
            elements,
            meta,
            element_judged,
            meta_judged,
            element_rejected,
            meta_rejected,
        })
    }

    async fn integrate(
        &mut self,
        hash: DhtOpHash,
        v: IntegratedDhtOpsValue,
    ) -> DhtOpConvertResult<()> {
        disintegrate_single_metadata(v.op.clone(), &self.element_judged, &mut self.meta_judged)
            .await?;
        disintegrate_single_op(v.op.clone(), &mut self.element_judged);
        self.integrated_dht_ops.put(hash, v)?;
        Ok(())
    }

    pub fn op_exists(&self, hash: &DhtOpHash) -> DatabaseResult<bool> {
        Ok(self.integrated_dht_ops.contains(&hash)? || self.integration_limbo.contains(&hash)?)
    }
}
