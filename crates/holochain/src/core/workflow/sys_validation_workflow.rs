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
    dht_op::DhtOp, header::NewEntryHeaderRef, test_utils::which_agent, validate::ValidationStatus,
    Entry, Timestamp,
};
use holochain_zome_types::{
    header::{ElementDelete, EntryType, EntryUpdate, LinkAdd, LinkRemove},
    Header,
};
use std::convert::TryInto;
use tracing::*;

use types::{DhtOpOrder, Outcome};

mod types;

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
    writer
        .with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))
        .await?;

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
    let mut ops: Vec<ValidationLimboValue> = fresh_reader!(env, |r| workspace
        .validation_limbo
        .drain_iter_filter(&r, |(_, vlv)| {
            match vlv.status {
                // We only want pending or awaiting sys dependency ops
                ValidationLimboStatus::Pending | ValidationLimboStatus::AwaitingSysDeps(_) => {
                    Ok(true)
                }
                ValidationLimboStatus::SysValidated | ValidationLimboStatus::AwaitingAppDeps(_) => {
                    Ok(false)
                }
            }
        })?
        .collect())?;

    // Sort the ops
    ops.sort_unstable_by_key(|v| DhtOpOrder::from(&v.op));

    debug!(
        agent = %which_agent(conductor_api.cell_id().agent_pubkey()),
        ?ops
    );

    // Process each op
    for mut vlv in ops {
        let outcome = validate_op(&vlv.op, workspace, network.clone(), &conductor_api).await?;

        // TODO: When we introduce abandoning ops make
        // sure they are not written to any outgoing
        // database

        match outcome {
            Outcome::Accepted => {
                vlv.status = ValidationLimboStatus::SysValidated;
                to_val_limbo(vlv, workspace).await?;
            }
            Outcome::SkipAppValidation => {
                let iv = IntegrationLimboValue {
                    op: vlv.op,
                    validation_status: ValidationStatus::Valid,
                };
                to_int_limbo(iv, workspace).await?;
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
                //
                // We might be able to make sure the `missing_dep` hash below
                // is always the correct dht basis hash for the authorities and
                // then request gossip off that authority.
                // However we are that authority by definition so maybe we should
                // just trigger a general gossip fetch at this point?
                vlv.status = ValidationLimboStatus::AwaitingSysDeps(missing_dep);
                to_val_limbo(vlv, workspace).await?;
            }
            Outcome::MissingDhtDep => {
                vlv.status = ValidationLimboStatus::Pending;
                to_val_limbo(vlv, workspace).await?;
            }
            Outcome::Rejected => {
                let iv = IntegrationLimboValue {
                    op: vlv.op,
                    validation_status: ValidationStatus::Rejected,
                };
                to_int_limbo(iv, workspace).await?;
            }
        }
    }
    Ok(WorkComplete::Complete)
}

async fn to_val_limbo(
    mut vlv: ValidationLimboValue,
    workspace: &mut SysValidationWorkspace,
) -> WorkflowResult<()> {
    let hash = DhtOpHash::with_data(&vlv.op).await;
    vlv.last_try = Some(Timestamp::now());
    vlv.num_tries += 1;
    workspace.validation_limbo.put(hash, vlv)?;
    Ok(())
}

async fn to_int_limbo(
    iv: IntegrationLimboValue,
    workspace: &mut SysValidationWorkspace,
) -> WorkflowResult<()> {
    let hash = DhtOpHash::with_data(&iv.op).await;
    workspace.integration_limbo.put(hash, iv)?;
    Ok(())
}

async fn validate_op(
    op: &DhtOp,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    conductor_api: &impl CellConductorApiT,
) -> WorkflowResult<Outcome> {
    match validate_op_inner(op, workspace, network, conductor_api).await {
        Ok(_) => match op {
            DhtOp::RegisterAgentActivity(_, _) |
            // TODO: Check strict mode where store element 
            // is also run through app validation
            DhtOp::StoreElement(_, _, _) => Ok(Outcome::SkipAppValidation),
            _ => Ok(Outcome::Accepted)
        },
        // Handle the errors that result in pending or awaiting deps
        Err(SysValidationError::ValidationError(e)) => {
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
fn handle_failed(error: ValidationError) -> Outcome {
    use Outcome::*;
    match error {
        ValidationError::DepMissingFromDht(_) => MissingDhtDep,
        ValidationError::DnaMissing(cell_id) => {
            panic!("Cell {:?} is missing the Dna code", cell_id)
        }
        ValidationError::EntryDefId(_) => Rejected,
        ValidationError::EntryHash => Rejected,
        ValidationError::EntryTooLarge(_, _) => Rejected,
        ValidationError::EntryType => Rejected,
        ValidationError::EntryVisibility(_) => Rejected,
        ValidationError::TagTooLarge(_, _) => Rejected,
        ValidationError::NotLinkAdd(_) => Rejected,
        ValidationError::NotNewEntry(_) => Rejected,
        ValidationError::NotHoldingDep(dep) => AwaitingOpDep(dep),
        ValidationError::PrevHeaderError(PrevHeaderError::MissingMeta(dep)) => {
            AwaitingOpDep(dep.into())
        }
        ValidationError::PrevHeaderError(_) => Rejected,
        ValidationError::PrivateEntry => Rejected,
        ValidationError::UpdateTypeMismatch(_, _) => Rejected,
        ValidationError::VerifySignature(_, _) => Rejected,
        ValidationError::ZomeId(_) => Rejected,
    }
}

async fn validate_op_inner(
    op: &DhtOp,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
    conductor_api: &impl CellConductorApiT,
) -> SysValidationResult<()> {
    match op {
        DhtOp::StoreElement(signature, header, entry) => {
            store_element(header, workspace.cascade(network.clone())).await?;
            if let Some(entry) = entry {
                store_entry(
                    (header)
                        .try_into()
                        .map_err(|_| ValidationError::NotNewEntry(header.clone()))?,
                    entry.as_ref(),
                    conductor_api,
                    workspace.cascade(network),
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
                workspace.cascade(network.clone()),
            )
            .await?;

            let header = header.clone().into();
            store_element(&header, workspace.cascade(network)).await?;
            all_op_check(signature, &header).await?;
            Ok(())
        }
        DhtOp::RegisterAgentActivity(signature, header) => {
            register_agent_activity(header, workspace).await?;

            all_op_check(signature, header).await?;
            Ok(())
        }
        DhtOp::RegisterUpdatedBy(signature, header) => {
            register_updated_by(header, workspace).await?;

            let header = header.clone().into();
            all_op_check(signature, &header).await?;
            Ok(())
        }
        DhtOp::RegisterDeletedBy(signature, header)
        | DhtOp::RegisterDeletedEntryHeader(signature, header) => {
            register_deleted(header, &workspace.element_vault).await?;

            let header = header.clone().into();
            all_op_check(signature, &header).await?;
            Ok(())
        }
        DhtOp::RegisterAddLink(signature, header) => {
            register_add_link(header, workspace, network).await?;

            let header = header.clone().into();
            all_op_check(signature, &header).await?;
            Ok(())
        }
        DhtOp::RegisterRemoveLink(signature, header) => {
            register_remove_link(header, workspace).await?;

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
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let author = header.author();
    let prev_header_hash = header.prev_header();

    // Checks
    check_prev_header(&header)?;
    check_valid_if_dna(&header, &workspace.meta_vault).await?;
    if let Some(prev_header_hash) = prev_header_hash {
        check_holding_prev_header(
            author.clone(),
            prev_header_hash,
            &workspace.meta_vault,
            &workspace.element_vault,
        )
        .await?;
    }
    check_chain_rollback(&header, &workspace.meta_vault, &workspace.element_vault).await?;
    Ok(())
}

async fn store_element(header: &Header, cascade: Cascade<'_>) -> SysValidationResult<()> {
    // Get data ready to validate
    let prev_header_hash = header.prev_header();

    // Checks
    check_prev_header(header)?;
    if let Some(prev_header_hash) = prev_header_hash {
        let prev_header = check_header_exists(prev_header_hash.clone(), cascade).await?;
        check_prev_timestamp(&header, prev_header.header())?;
        check_prev_seq(&header, prev_header.header())?;
    }
    Ok(())
}

async fn store_entry(
    header: NewEntryHeaderRef<'_>,
    entry: &Entry,
    conductor_api: &impl CellConductorApiT,
    cascade: Cascade<'_>,
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

    // Additional checks if this is an EntryUpdate
    if let NewEntryHeaderRef::Update(entry_update) = header {
        let original_header =
            check_header_exists(entry_update.original_header_address.clone(), cascade).await?;
        update_check(entry_update, original_header.header())?;
    }
    Ok(())
}

async fn register_updated_by(
    entry_update: &EntryUpdate,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_header_address = &entry_update.original_header_address;
    let original_entry_address = entry_update.original_entry_address.clone();

    // Checks
    check_header_in_metadata(
        original_entry_address,
        original_header_address,
        &workspace.meta_vault,
    )
    .await?;
    let original_element =
        check_holding_element(original_header_address, &workspace.element_vault).await?;
    update_check(entry_update, original_element.header())?;
    Ok(())
}

async fn register_deleted(
    element_delete: &ElementDelete,
    element_vault: &ElementBuf,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_header_address = &element_delete.removes_address;

    // Checks
    let removed_header = check_holding_header(removed_header_address, element_vault).await?;
    check_new_entry_header(removed_header.header())?;
    Ok(())
}

async fn register_add_link(
    link_add: &LinkAdd,
    workspace: &mut SysValidationWorkspace,
    network: HolochainP2pCell,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let base_entry_address = &link_add.base_address;
    let target_entry_address = &link_add.target_address;

    // Checks
    check_holding_entry(base_entry_address, &workspace.element_vault).await?;
    check_entry_exists(target_entry_address.clone(), workspace.cascade(network)).await?;
    check_tag_size(&link_add.tag)?;
    Ok(())
}

async fn register_remove_link(
    link_remove: &LinkRemove,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let link_add_address = &link_remove.link_add_address;

    // Checks
    let link_add = check_holding_header(link_add_address, &workspace.element_vault).await?;
    let (link_add, link_add_hash) = link_add.into_header_and_signature().0.into_inner();
    check_link_in_metadata(link_add, &link_add_hash, &workspace.meta_vault).await?;
    Ok(())
}

fn update_check(entry_update: &EntryUpdate, original_header: &Header) -> SysValidationResult<()> {
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
    pub element_vault: ElementBuf,
    pub meta_vault: MetadataBuf,
    pub element_cache: ElementBuf,
    pub meta_cache: MetadataBuf,
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
    pub fn new(env: EnvironmentRead, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBufFresh::new(env.clone(), db);

        let validation_limbo = ValidationLimboStore::new(env.clone(), dbs)?;

        let element_vault = ElementBuf::vault(env.clone(), dbs, false)?;
        let meta_vault = MetadataBuf::vault(env.clone(), dbs)?;
        let element_cache = ElementBuf::cache(env.clone(), dbs)?;
        let meta_cache = MetadataBuf::cache(env, dbs)?;

        Ok(Self {
            integration_limbo,
            validation_limbo,
            element_vault,
            meta_vault,
            element_cache,
            meta_cache,
        })
    }
}

impl Workspace for SysValidationWorkspace {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.validation_limbo.0.flush_to_txn(writer)?;
        self.integration_limbo.flush_to_txn(writer)?;
        // Flush for cascade
        self.element_cache.flush_to_txn(writer)?;
        self.meta_cache.flush_to_txn(writer)?;
        Ok(())
    }
}
