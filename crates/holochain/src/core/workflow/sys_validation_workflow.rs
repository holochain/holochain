//! The workflow and queue consumer for sys validation

use super::*;
use crate::{
    conductor::api::CellConductorApiT,
    core::{
        queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
        state::{
            cascade::Cascade,
            dht_op_integration::{IntegrationLimboStore, IntegrationLimboValue},
            element_buf::ElementBuf,
            metadata::MetadataBuf,
            validation_db::{ValidationLimboStatus, ValidationLimboStore, ValidationLimboValue},
            workspace::{Workspace, WorkspaceResult},
        },
        sys_validate::*,
        validation::*,
    },
};
use error::WorkflowResult;
use fallible_iterator::FallibleIterator;
use holo_hash::DhtOpHash;
use holochain_keystore::Signature;
use holochain_p2p::HolochainP2pCell;
use holochain_state::{
    buffer::{BufferedStore, KvBufFresh},
    db::INTEGRATION_LIMBO,
    fresh_reader,
    prelude::*,
};
use holochain_types::{
    dht_op::DhtOp, dht_op::DhtOpLight, header::NewEntryHeaderRef, test_utils::which_agent,
    validate::ValidationStatus, Entry, Timestamp,
};
use holochain_zome_types::{
    header::{CreateLink, Delete, DeleteLink, EntryType, Update},
    Header,
};
use std::{collections::BinaryHeap, convert::TryInto};
use tracing::*;

use integrate_dht_ops_workflow::{
    disintegrate_single_data, disintegrate_single_metadata, integrate_single_data,
    integrate_single_metadata, reintegrate_single_data,
};
use produce_dht_ops_workflow::dht_op_light::light_to_op;
use types::{DhtOpOrder, OrderedOp, Outcome};

pub mod types;

#[cfg(test)]
mod tests;

#[instrument(skip(workspace, writer, trigger_app_validation, network, conductor_api))]
pub async fn sys_validation_workflow(
    mut workspace: SysValidationWorkspace,
    writer: OneshotWriter,
    trigger_app_validation: &mut TriggerSender,
    network: HolochainP2pCell,
    conductor_api: impl CellConductorApiT,
) -> WorkflowResult<WorkComplete> {
    let complete = sys_validation_workflow_inner(&mut workspace, network, conductor_api).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer.with_writer(|writer| Ok(workspace.flush_to_txn_ref(writer)?))?;

    // trigger other workflows
    trigger_app_validation.trigger();

    Ok(complete)
}

async fn sys_validation_workflow_inner(
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    conductor_api: impl CellConductorApiT,
) -> WorkflowResult<WorkComplete> {
    let env = workspace.validation_limbo.env().clone();
    // Drain all the ops
    let ops: Vec<ValidationLimboValue> = fresh_reader!(env, |r| workspace
        .validation_limbo
        .drain_iter_filter(&r, |(_, vlv)| {
            match vlv.status {
                // We only want pending or awaiting sys dependency ops
                ValidationLimboStatus::Pending | ValidationLimboStatus::AwaitingSysDeps(_) => {
                    Ok(true)
                }
                ValidationLimboStatus::SysValidated
                | ValidationLimboStatus::AwaitingAppDeps(_)
                | ValidationLimboStatus::PendingValidation => Ok(false),
            }
        })?
        .collect())?;

    // Sort the ops
    let mut sorted_ops = BinaryHeap::new();
    for vlv in ops {
        let op = light_to_op(vlv.op.clone(), &workspace.element_pending).await?;

        let hash = DhtOpHash::with_data_sync(&op);
        let order = DhtOpOrder::from(&op);
        let v = OrderedOp {
            order,
            hash,
            op,
            value: vlv,
        };
        // We want a min-heap
        sorted_ops.push(std::cmp::Reverse(v));

        // Since we are processing DhtOps in a loop, make sure we yield
        // between each one, since hashing could take a while
        tokio::task::yield_now().await;
    }

    // Process each op
    for so in sorted_ops {
        let OrderedOp {
            hash: op_hash,
            op,
            value: mut vlv,
            ..
        } = so.0;
        let outcome = validate_op(
            &op,
            workspace,
            network.clone(),
            &conductor_api,
            &mut vlv.pending_dependencies,
            CheckLevel::Proof,
        )
        .await?;

        match outcome {
            Outcome::Accepted => {
                vlv.status = ValidationLimboStatus::SysValidated;
                workspace.put_val_limbo(op_hash, vlv)?;
            }
            Outcome::SkipAppValidation => {
                if vlv.pending_dependencies.pending_dependencies() {
                    vlv.status = ValidationLimboStatus::PendingValidation;
                    workspace.put_val_limbo(op_hash, vlv)?;
                } else {
                    let iv = IntegrationLimboValue {
                        op: vlv.op,
                        validation_status: ValidationStatus::Valid,
                    };
                    workspace.put_int_limbo(op_hash, iv, op)?;
                }
            }
            Outcome::AwaitingOpDep(missing_dep) => {
                // TODO: Try and get this dependency to add to limbo
                //
                // I actually can't see how we can do this because there's no
                // way to get an DhtOpHash without either having the op or the full
                // header. We have neither that's why where here.
                //
                // We need to be holding the dependency because
                // we were meant to get a StoreElement or StoreEntry or
                // RegisterAgentActivity or RegisterAddLink.
                vlv.status = ValidationLimboStatus::AwaitingSysDeps(missing_dep);
                workspace.put_val_limbo(op_hash, vlv)?;
            }
            Outcome::MissingDhtDep => {
                vlv.status = ValidationLimboStatus::Pending;
                workspace.put_val_limbo(op_hash, vlv)?;
            }
            Outcome::Rejected => {
                let iv = IntegrationLimboValue {
                    op: vlv.op,
                    validation_status: ValidationStatus::Rejected,
                };
                workspace.put_int_limbo(op_hash, iv, op)?;
            }
        }
    }
    Ok(WorkComplete::Complete)
}

async fn validate_op(
    op: &DhtOp,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    conductor_api: &impl CellConductorApiT,
    dependencies: &mut PendingDependencies,
    check_level: CheckLevel,
) -> WorkflowResult<Outcome> {
    match validate_op_inner(
        op,
        workspace,
        network,
        conductor_api,
        dependencies,
        check_level,
    )
    .await
    {
        Ok(_) => match op {
            // TODO: Check strict mode where store element
            // is also run through app validation
            DhtOp::RegisterAgentActivity(_, _) => Ok(Outcome::SkipAppValidation),
            _ => Ok(Outcome::Accepted),
        },
        // Handle the errors that result in pending or awaiting deps
        Err(SysValidationError::ValidationOutcome(e)) => {
            warn!(
                agent = %which_agent(conductor_api.cell_id().agent_pubkey()),
                msg = "DhtOp has failed system validation",
                ?op,
                error = ?e,
                error_msg = %e
            );
            Ok(handle_failed(e))
        }
        Err(e) => Err(e.into()),
    }
}

/// For now errors result in an outcome but in the future
/// we might find it useful to include the reason something
/// was rejected etc.
/// This is why the errors contain data but is currently unread.
fn handle_failed(error: ValidationOutcome) -> Outcome {
    use Outcome::*;
    match error {
        ValidationOutcome::DepMissingFromDht(_) => MissingDhtDep,
        ValidationOutcome::EntryDefId(_) => Rejected,
        ValidationOutcome::EntryHash => Rejected,
        ValidationOutcome::EntryTooLarge(_, _) => Rejected,
        ValidationOutcome::EntryType => Rejected,
        ValidationOutcome::EntryVisibility(_) => Rejected,
        ValidationOutcome::TagTooLarge(_, _) => Rejected,
        ValidationOutcome::NotCreateLink(_) => Rejected,
        ValidationOutcome::NotNewEntry(_) => Rejected,
        ValidationOutcome::NotHoldingDep(dep) => AwaitingOpDep(dep),
        ValidationOutcome::PrevHeaderError(PrevHeaderError::MissingMeta(dep)) => {
            AwaitingOpDep(dep.into())
        }
        ValidationOutcome::PrevHeaderError(_) => Rejected,
        ValidationOutcome::PrivateEntry => Rejected,
        ValidationOutcome::UpdateTypeMismatch(_, _) => Rejected,
        ValidationOutcome::VerifySignature(_, _) => Rejected,
        ValidationOutcome::ZomeId(_) => Rejected,
    }
}

async fn validate_op_inner(
    op: &DhtOp,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    conductor_api: &impl CellConductorApiT,
    dependencies: &mut PendingDependencies,
    check_level: CheckLevel,
) -> SysValidationResult<()> {
    match op {
        DhtOp::StoreElement(signature, header, entry) => {
            store_element(header, workspace, network.clone(), dependencies).await?;
            if let Some(entry) = entry {
                store_entry(
                    (header)
                        .try_into()
                        .map_err(|_| ValidationOutcome::NotNewEntry(header.clone()))?,
                    entry.as_ref(),
                    conductor_api,
                    workspace,
                    network,
                    dependencies,
                )
                .await?;
            }

            all_op_check(signature, header).await?;
            Ok(())
        }
        DhtOp::StoreEntry(signature, header, entry) => {
            store_entry(
                (header).into(),
                entry.as_ref(),
                conductor_api,
                workspace,
                network.clone(),
                dependencies,
            )
            .await?;

            let header = header.clone().into();
            store_element(&header, workspace, network, dependencies).await?;
            all_op_check(signature, &header).await?;
            Ok(())
        }
        DhtOp::RegisterAgentActivity(signature, header) => {
            register_agent_activity(
                header,
                workspace,
                network.clone(),
                dependencies,
                check_level,
            )
            .await?;
            store_element(header, workspace, network, dependencies).await?;
            all_op_check(signature, header).await?;
            Ok(())
        }
        DhtOp::RegisterUpdatedBy(signature, header) => {
            register_updated_by(header, workspace, network, dependencies, check_level).await?;

            let header = header.clone().into();
            all_op_check(signature, &header).await?;
            Ok(())
        }
        DhtOp::RegisterDeletedBy(signature, header) => {
            register_deleted_by(header, workspace, network, dependencies, check_level).await?;

            let header = header.clone().into();
            all_op_check(signature, &header).await?;
            Ok(())
        }
        DhtOp::RegisterDeletedEntryHeader(signature, header) => {
            register_deleted_entry_header(header, workspace, network, dependencies, check_level)
                .await?;

            let header = header.clone().into();
            all_op_check(signature, &header).await?;
            Ok(())
        }
        DhtOp::RegisterAddLink(signature, header) => {
            register_add_link(header, workspace, network, dependencies, check_level).await?;

            let header = header.clone().into();
            all_op_check(signature, &header).await?;
            Ok(())
        }
        DhtOp::RegisterRemoveLink(signature, header) => {
            register_delete_link(header, workspace, network, dependencies, check_level).await?;

            let header = header.clone().into();
            all_op_check(signature, &header).await?;
            Ok(())
        }
    }
}

async fn all_op_check(signature: &Signature, header: &Header) -> SysValidationResult<()> {
    verify_header_signature(&signature, &header).await?;
    author_key_is_valid(header.author()).await?;
    Ok(())
}

async fn register_agent_activity(
    header: &Header,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    dependencies: &mut PendingDependencies,
    check_level: CheckLevel,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let author = header.author();
    let prev_header_hash = header.prev_header();

    // Checks
    check_prev_header(&header)?;
    check_valid_if_dna(&header, &workspace.meta_vault).await?;
    if let Some(prev_header_hash) = prev_header_hash {
        let dependency = check_holding_prev_header_all(
            author,
            prev_header_hash,
            workspace,
            network,
            check_level,
        )
        .await?;
        dependencies.register_agent_activity(dependency);
    }
    check_chain_rollback(&header, &workspace.meta_vault, &workspace.element_vault).await?;
    Ok(())
}

async fn store_element(
    header: &Header,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    dependencies: &mut PendingDependencies,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let prev_header_hash = header.prev_header();

    // Checks
    check_prev_header(header)?;
    if let Some(prev_header_hash) = prev_header_hash {
        let dependency = check_header_exists(prev_header_hash.clone(), workspace, network).await?;
        let prev_header = dependencies.store_element(dependency);
        check_prev_timestamp(&header, prev_header.header())?;
        check_prev_seq(&header, prev_header.header())?;
    }
    Ok(())
}

async fn store_entry(
    header: NewEntryHeaderRef<'_>,
    entry: &Entry,
    conductor_api: &impl CellConductorApiT,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    dependencies: &mut PendingDependencies,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let entry_type = header.entry_type();
    let entry_hash = header.entry_hash();

    // Checks
    check_entry_type(entry_type, entry)?;
    if let EntryType::App(app_entry_type) = entry_type {
        let entry_def = check_app_entry_type(app_entry_type, conductor_api).await?;
        check_not_private(&entry_def)?;
    }
    check_entry_hash(entry_hash, entry).await?;
    check_entry_size(entry)?;

    // Additional checks if this is an Update
    if let NewEntryHeaderRef::Update(entry_update) = header {
        let dependency = check_header_exists(
            entry_update.original_header_address.clone(),
            workspace,
            network,
        )
        .await?;
        let original_header = dependencies.store_element(dependency);
        update_check(entry_update, original_header.header())?;
    }
    Ok(())
}

async fn register_updated_by(
    entry_update: &Update,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    dependencies: &mut PendingDependencies,
    check_level: CheckLevel,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_header_address = &entry_update.original_header_address;
    let original_entry_address = &entry_update.original_entry_address;

    let dependency = check_holding_store_entry_all(
        original_entry_address,
        original_header_address,
        workspace,
        network,
        check_level,
    )
    .await?;
    let original_element = dependencies
        .store_entry_fixed(dependency)
        .ok_or_else(|| ValidationOutcome::not_holding(original_header_address))?;
    update_check(entry_update, original_element.header())?;
    Ok(())
}

async fn register_deleted_by(
    element_delete: &Delete,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    dependencies: &mut PendingDependencies,
    check_level: CheckLevel,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_header_address = &element_delete.deletes_address;

    // Checks
    let dependency =
        check_holding_element_all(removed_header_address, workspace, network, check_level).await?;
    let removed_header = dependencies
        .store_entry_fixed(dependency)
        .ok_or_else(|| ValidationOutcome::not_holding(removed_header_address))?;
    check_new_entry_header(removed_header.header())?;
    Ok(())
}

async fn register_deleted_entry_header(
    element_delete: &Delete,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    dependencies: &mut PendingDependencies,
    check_level: CheckLevel,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_header_address = &element_delete.deletes_address;

    // Checks
    let dependency =
        check_holding_header_all(removed_header_address, workspace, network, check_level).await?;
    let removed_header = dependencies.store_element(dependency);
    check_new_entry_header(removed_header.header())?;
    Ok(())
}

async fn register_add_link(
    link_add: &CreateLink,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    dependencies: &mut PendingDependencies,
    check_level: CheckLevel,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let base_entry_address = &link_add.base_address;
    let target_entry_address = &link_add.target_address;

    // Checks
    let dependency =
        check_holding_entry_all(base_entry_address, workspace, network.clone(), check_level)
            .await?;
    dependencies
        .store_entry_any(dependency)
        .ok_or_else(|| ValidationOutcome::not_holding(base_entry_address))?;
    let dependency = check_entry_exists(target_entry_address.clone(), workspace, network).await?;
    dependencies
        .store_entry_any(dependency)
        .ok_or_else(|| ValidationOutcome::not_found(target_entry_address))?;
    check_tag_size(&link_add.tag)?;
    Ok(())
}

async fn register_delete_link(
    link_remove: &DeleteLink,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    dependencies: &mut PendingDependencies,
    check_level: CheckLevel,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let link_add_address = &link_remove.link_add_address;

    // Checks
    let dependency =
        check_holding_link_add_all(link_add_address, workspace, network, check_level).await?;
    dependencies
        .add_link(dependency)
        .ok_or_else(|| ValidationOutcome::not_holding(link_add_address))?;
    Ok(())
}

fn update_check(entry_update: &Update, original_header: &Header) -> SysValidationResult<()> {
    check_new_entry_header(original_header)?;
    let original_header: NewEntryHeaderRef = original_header
        .try_into()
        .expect("This can't fail due to the above check_new_entry_header");
    check_update_reference(entry_update, &original_header)?;
    Ok(())
}

pub struct SysValidationWorkspace {
    pub integration_limbo: IntegrationLimboStore,
    pub validation_limbo: ValidationLimboStore,
    // Integrated data
    pub element_vault: ElementBuf,
    pub meta_vault: MetadataBuf,
    // Data pending validation
    pub element_pending: ElementBuf<PendingPrefix>,
    pub meta_pending: MetadataBuf<PendingPrefix>,
    // Data that has progressed past validation and is pending Integration
    pub element_judged: ElementBuf<JudgedPrefix>,
    pub meta_judged: MetadataBuf<JudgedPrefix>,
    // Cached data
    pub element_cache: ElementBuf,
    pub meta_cache: MetadataBuf,
    // Ops to disintegrate
    pub to_disintegrate_pending: Vec<DhtOpLight>,
}

impl<'a> SysValidationWorkspace {
    pub fn cascade(&'a mut self, network: HolochainP2pCell) -> Cascade<'a> {
        Cascade::new(
            self.validation_limbo.env().clone(),
            &self.element_vault,
            &self.meta_vault,
            &mut self.element_cache,
            &mut self.meta_cache,
            network,
        )
    }
}

impl SysValidationWorkspace {
    pub fn new(env: EnvironmentRead) -> WorkspaceResult<Self> {
        let db = env.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBufFresh::new(env.clone(), db);

        let validation_limbo = ValidationLimboStore::new(env.clone())?;

        let element_vault = ElementBuf::vault(env.clone(), false)?;
        let meta_vault = MetadataBuf::vault(env.clone())?;
        let element_cache = ElementBuf::cache(env.clone())?;
        let meta_cache = MetadataBuf::cache(env.clone())?;

        let element_pending = ElementBuf::pending(env.clone())?;
        let meta_pending = MetadataBuf::pending(env.clone())?;

        let element_judged = ElementBuf::judged(env.clone())?;
        let meta_judged = MetadataBuf::judged(env)?;

        Ok(Self {
            integration_limbo,
            validation_limbo,
            element_vault,
            meta_vault,
            element_pending,
            meta_pending,
            element_judged,
            meta_judged,
            element_cache,
            meta_cache,
            to_disintegrate_pending: Vec::new(),
        })
    }

    fn put_val_limbo(
        &mut self,
        hash: DhtOpHash,
        mut vlv: ValidationLimboValue,
    ) -> WorkflowResult<()> {
        vlv.last_try = Some(Timestamp::now());
        vlv.num_tries += 1;
        self.validation_limbo.put(hash, vlv)?;
        Ok(())
    }

    #[tracing::instrument(skip(self, hash, op))]
    fn put_int_limbo(
        &mut self,
        hash: DhtOpHash,
        iv: IntegrationLimboValue,
        op: DhtOp,
    ) -> WorkflowResult<()> {
        disintegrate_single_metadata(iv.op.clone(), &self.element_pending, &mut self.meta_pending)?;
        self.to_disintegrate_pending.push(iv.op.clone());
        integrate_single_data(op, &mut self.element_judged)?;
        integrate_single_metadata(iv.op.clone(), &self.element_judged, &mut self.meta_judged)?;
        self.integration_limbo.put(hash, iv)?;
        Ok(())
    }

    #[tracing::instrument(skip(self, writer))]
    /// We need to cancel any deletes for the pending data
    /// where the ops still in validation limbo reference that data
    fn update_element_stores(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        for op in self.to_disintegrate_pending.drain(..) {
            disintegrate_single_data(op, &mut self.element_pending);
        }
        let mut val_iter = self.validation_limbo.iter(writer)?;
        while let Some((_, vlv)) = val_iter.next()? {
            reintegrate_single_data(vlv.op, &mut self.element_pending);
        }
        Ok(())
    }
}

impl Workspace for SysValidationWorkspace {
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.update_element_stores(writer)?;
        self.validation_limbo.0.flush_to_txn_ref(writer)?;
        self.integration_limbo.flush_to_txn_ref(writer)?;
        // Flush for cascade
        self.element_cache.flush_to_txn_ref(writer)?;
        self.meta_cache.flush_to_txn_ref(writer)?;

        self.element_pending.flush_to_txn_ref(writer)?;
        self.meta_pending.flush_to_txn_ref(writer)?;
        self.element_judged.flush_to_txn_ref(writer)?;
        self.meta_judged.flush_to_txn_ref(writer)?;
        Ok(())
    }
}
