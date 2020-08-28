//! The workflow and queue consumer for DhtOp integration

use super::*;
use crate::core::{
    queue_consumer::{OneshotWriter, WorkComplete},
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
    dht_op::{produce_op_lights_from_elements, DhtOp, DhtOpHashed, DhtOpLight},
    element::{Element, SignedHeaderHashed, SignedHeaderHashedExt},
    validate::ValidationStatus,
    Entry, EntryHashed, Timestamp,
};
use holochain_zome_types::{element::SignedHeader, Header};
use produce_dht_ops_workflow::dht_op_light::error::{DhtOpConvertError, DhtOpConvertResult};
use std::convert::TryInto;
use tracing::*;

mod tests;

#[instrument(skip(workspace, writer))]
pub async fn integrate_dht_ops_workflow(
    mut workspace: IntegrateDhtOpsWorkspace,
    writer: OneshotWriter,
) -> WorkflowResult<WorkComplete> {
    // one of many possible ways to access the env
    let env = workspace.elements.headers().env().clone();
    // Pull ops out of queue
    // TODO: PERF: we collect() only because this iterator cannot cross awaits,
    // but is there a way to do this without collect()?
    let ops: Vec<DatabaseResult<IntegrationLimboValue>> =
        fresh_reader!(env, |r| DatabaseResult::Ok(
            workspace
                .integration_limbo
                .drain_iter(&r)?
                .iterator()
                .collect::<Vec<_>>()
        ))?;

    // Compute hashes for all dht_ops, include in tuples alongside db values
    let mut ops = futures::future::join_all(
        ops.into_iter()
            .map(|val| {
                val.map(|val| async move {
                    let IntegrationLimboValue {
                        op,
                        validation_status,
                    } = val;
                    let (op, op_hash) = DhtOpHashed::from_content(op).into_inner();
                    (
                        op_hash,
                        IntegrationLimboValue {
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
            if workspace.integrated_dht_ops.get(&op_hash).await?.is_none() {
                match integrate_single_dht_op(value, &mut workspace.elements, &mut workspace.meta)
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
            workspace.integration_limbo.put(op_hash, value)?;
        }
        WorkComplete::Incomplete
    };

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))
        .await?;

    // trigger other workflows

    // TODO: Add trigger validation here
    if total_integrated > 0 {}

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
    value: IntegrationLimboValue,
    element_store: &mut ElementBuf,
    meta_store: &mut MetadataBuf,
) -> DhtOpConvertResult<Outcome> {
    match integrate_single_element(value, element_store).await? {
        Outcome::Integrated(v) => {
            integrate_single_metadata(v.op.clone(), element_store, meta_store).await?;
            debug!("integrating");
            Ok(Outcome::Integrated(v))
        }
        v @ Outcome::Deferred(_) => Ok(v),
    }
}

async fn integrate_single_element(
    value: IntegrationLimboValue,
    element_store: &mut ElementBuf,
) -> DhtOpConvertResult<Outcome> {
    {
        // Process each op
        let IntegrationLimboValue {
            op,
            validation_status,
        } = value;
        let light_op = op.to_light().await;

        async fn put_data(
            signature: Signature,
            header: Header,
            maybe_entry: Option<Entry>,
            element_store: &mut ElementBuf,
        ) -> DhtOpConvertResult<()> {
            let signed_header = SignedHeaderHashed::from_content(SignedHeader(header, signature));
            let maybe_entry_hashed = match maybe_entry {
                Some(entry) => Some(EntryHashed::from_content(entry)),
                None => None,
            };
            element_store.put(signed_header, maybe_entry_hashed)?;
            Ok(())
        }

        async fn header_with_entry_is_stored(
            hash: &HeaderHash,
            element_store: &ElementBuf,
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

        let entry_is_stored = |hash| element_store.contains_entry(hash);

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
                    return Outcome::deferred(op, validation_status);
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
                    return Outcome::deferred(op, validation_status);
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
                    return Outcome::deferred(op, validation_status);
                }
                put_data(signature, element_delete.into(), None, element_store).await?;
            }
            DhtOp::RegisterAddLink(signature, link_add) => {
                // Check whether we have the base address in the Vault.
                // If not then this should put the op back on the queue.
                if !entry_is_stored(&link_add.base_address).await? {
                    let op = DhtOp::RegisterAddLink(signature, link_add);
                    return Outcome::deferred(op, validation_status);
                }

                put_data(signature, link_add.into(), None, element_store).await?;
            }
            DhtOp::RegisterRemoveLink(signature, link_remove) => {
                // Check whether we have the base address and link add address
                // are in the Vault.
                // If not then this should put the op back on the queue.
                if !entry_is_stored(&link_remove.base_address).await?
                    || !header_is_stored(&link_remove.link_add_address).await?
                {
                    let op = DhtOp::RegisterRemoveLink(signature, link_remove);
                    return Outcome::deferred(op, validation_status);
                }

                put_data(signature, link_remove.into(), None, element_store).await?;
            }
        }

        let value = IntegratedDhtOpsValue {
            validation_status,
            op: light_op,
            when_integrated: Timestamp::now(),
        };
        Ok(Outcome::Integrated(value))
    }
}

pub async fn integrate_single_metadata<C: MetadataBufT>(
    op: DhtOpLight,
    element_store: &ElementBuf,
    meta_store: &mut C,
) -> DhtOpConvertResult<()> {
    async fn get_header(
        hash: HeaderHash,
        element_store: &ElementBuf,
    ) -> DhtOpConvertResult<Header> {
        Ok(element_store
            .get_header(&hash)
            .await?
            .ok_or(DhtOpConvertError::MissingData)?
            .into_header_and_signature()
            .0
            .into_content())
    }

    match op {
        DhtOpLight::StoreElement(_, _, _) => (),
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
    Deferred(IntegrationLimboValue),
}

impl Outcome {
    fn deferred(op: DhtOp, validation_status: ValidationStatus) -> DhtOpConvertResult<Self> {
        Ok(Outcome::Deferred(IntegrationLimboValue {
            op,
            validation_status,
        }))
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
        let meta = MetadataBuf::vault(env, dbs)?;

        Ok(Self {
            integration_limbo,
            integrated_dht_ops,
            elements,
            meta,
        })
    }

    pub async fn op_exists(&self, hash: &DhtOpHash) -> DatabaseResult<bool> {
        Ok(self.integrated_dht_ops.contains(&hash).await?
            || self.integration_limbo.contains(&hash).await?)
    }
}
