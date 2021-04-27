//! The workflow and queue consumer for DhtOp integration

use super::incoming_dht_ops_workflow::IncomingDhtOpsWorkspace;
use super::*;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use error::WorkflowResult;
use fallible_iterator::FallibleIterator;
use holo_hash::DhtOpHash;
use holo_hash::HeaderHash;
use holochain_cascade::Cascade;
use holochain_cascade::DbPair;
use holochain_conductor_api::IntegrationStateDump;
use holochain_sqlite::buffer::BufferedStore;
use holochain_sqlite::buffer::KvBufFresh;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::fresh_reader;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::prelude::*;

use holochain_zome_types::ValidationStatus;

use produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertResult;
use std::convert::TryInto;
use tracing::*;

pub use disintegrate::*;

mod disintegrate;

#[cfg(test)]
mod query_tests;
#[cfg(feature = "test_utils")]
mod tests;

#[instrument(skip(vault, trigger_sys, trigger_receipt))]
pub async fn integrate_dht_ops_workflow(
    vault: EnvWrite,
    mut trigger_sys: TriggerSender,
    mut trigger_receipt: TriggerSender,
) -> WorkflowResult<WorkComplete> {
    let time = holochain_types::timestamp::now();
    let changed = vault.conn()?.with_commit(|txn| {
        let dep = "
            SELECT 1 FROM Header AS H_DEP
            JOIN DhtOp AS OP_DEP ON OP_DEP.header_hash = H_DEP.hash 
            WHERE 
            OP_DEP.when_integrated IS NOT NULL
        ";
        let activity = format!(
            "{}
            AND Header.prev_hash = H_DEP.hash
            AND OP_DEP.type = :register_activity
            ",
            dep
        );
        let update_content = format!(
            "{}
			AND Header.original_header_hash = H_DEP.hash
			AND OP_DEP.type = :store_entry 
            ",
            dep
        );
        let update_element = format!(
            "{}
			AND Header.original_header_hash = H_DEP.hash
			AND OP_DEP.type = :store_element
            ",
            dep
        );
        let deleted_entry_header = format!(
            "{}
			AND Header.deletes_header_hash = H_DEP.hash
			AND OP_DEP.type = :store_entry
            ",
            dep
        );
        let deleted_by = format!(
            "{}
			AND Header.deletes_header_hash = H_DEP.hash
			AND OP_DEP.type = :store_element
            ",
            dep
        );
        let create_link = format!(
            "{}
			AND Header.base_hash = H_DEP.entry_hash
			AND OP_DEP.type = :store_entry
            ",
            dep
        );
        let delete_link = format!(
            "{}
			AND Header.create_link_hash = H_DEP.hash
			AND OP_DEP.type = :create_link
            ",
            dep
        );
        let ops = format!(
            "
            CASE DhtOp.type
                WHEN :store_entry               THEN 1
                WHEN :store_element             THEN 1
                WHEN :register_activity         THEN EXISTS({activity})
                WHEN :updated_content           THEN EXISTS({update_content})
                WHEN :updated_element           THEN EXISTS({update_element})
                WHEN :deleted_by                THEN EXISTS({deleted_by})
                WHEN :deleted_entry_header      THEN EXISTS({deleted_entry_header})
                WHEN :create_link               THEN EXISTS({create_link})
                WHEN :delete_link               THEN EXISTS({delete_link})
            END
            ",
            activity = activity,
            update_content = update_content,
            update_element = update_element,
            deleted_by = deleted_by,
            deleted_entry_header = deleted_entry_header,
            create_link = create_link,
            delete_link = delete_link,
        );
        let sql = format!(
            "
            UPDATE DhtOp
            SET
            when_integrated = :when_integrated,
            when_integrated_ns = :when_integrated_ns,
            validation_stage = NULL
            WHERE 
            validation_stage = 3
            AND
            DhtOp.header_hash IN (
                SELECT Header.hash
                FROM Header
                WHERE
                {}
            )
            ",
            ops
        );
        let mut stmt = txn.prepare(&sql)?;

        let changed = stmt.execute(
            // &sql,
            named_params! {
                ":when_integrated": time,
                ":when_integrated_ns": to_blob(time)?,
                ":store_entry": DhtOpType::StoreEntry,
                ":store_element": DhtOpType::StoreElement,
                ":register_activity": DhtOpType::RegisterAgentActivity,
                ":updated_content": DhtOpType::RegisterUpdatedContent,
                ":updated_element": DhtOpType::RegisterUpdatedElement,
                ":deleted_by": DhtOpType::RegisterDeletedBy,
                ":deleted_entry_header": DhtOpType::RegisterDeletedEntryHeader,
                ":create_link": DhtOpType::RegisterAddLink,
                ":delete_link": DhtOpType::RegisterRemoveLink,

            },
        )?;
        tracing::debug!("{}", stmt.expanded_sql().unwrap());
        WorkflowResult::Ok(changed)
    })?;
    tracing::debug!(?changed);
    if changed > 0 {
        trigger_sys.trigger();
        trigger_receipt.trigger();
        Ok(WorkComplete::Incomplete)
    } else {
        Ok(WorkComplete::Complete)
    }
}

/// Store a DhtOp's data in an element buf
pub fn integrate_single_data<P: PrefixType>(
    op: DhtOp,
    element_store: &mut ElementBuf<P>,
) -> DhtOpConvertResult<()> {
    todo!("remove this")
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
    todo!("remove this")
}

#[deprecated = "This is no longer needed, remove when updating tests"]
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
    pub fn new(env: EnvRead) -> WorkspaceResult<Self> {
        let db = env.get_table(TableName::IntegratedDhtOps)?;
        let integrated_dht_ops = KvBufFresh::new(env.clone(), db);

        let db = env.get_table(TableName::IntegrationLimbo)?;
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

pub fn dump_state(env: EnvRead) -> WorkspaceResult<IntegrationStateDump> {
    let workspace = IncomingDhtOpsWorkspace::new(env.clone())?;
    let (validation_limbo, integration_limbo, integrated) = fresh_reader!(env, |mut r| {
        let v = workspace.validation_limbo.iter(&mut r)?.count()?;
        let il = workspace.integration_limbo.iter(&mut r)?.count()?;
        let i = workspace.integrated_dht_ops.iter(&mut r)?.count()?;
        DatabaseResult::Ok((v, il, i))
    })?;

    Ok(IntegrationStateDump {
        validation_limbo,
        integration_limbo,
        integrated,
    })
}
