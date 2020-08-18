//! The workflow and queue consumer for sys validation

use super::*;
use crate::core::{
    queue_consumer::{OneshotWriter, TriggerSender, WorkComplete},
    state::{
        cascade::Cascade,
        dht_op_integration::{IntegratedDhtOpsStore, IntegrationLimboStore},
        element_buf::ElementBuf,
        metadata::MetadataBuf,
        validation_db::{ValidationLimboStatus, ValidationLimboStore, ValidationLimboValue},
        workspace::{Workspace, WorkspaceResult},
    },
    sys_validate::*,
};
use error::WorkflowResult;
use fallible_iterator::FallibleIterator;
use holo_hash::DhtOpHash;
use holochain_keystore::Signature;
use holochain_p2p::HolochainP2pCell;
use holochain_state::{
    buffer::{BufferedStore, KvBuf},
    db::{INTEGRATED_DHT_OPS, INTEGRATION_LIMBO},
    prelude::{GetDb, Reader, Writer},
};
use holochain_types::{dht_op::DhtOp, Timestamp};
use holochain_zome_types::Header;
use std::convert::TryInto;
use tracing::*;

#[instrument(skip(workspace, writer, trigger_app_validation, network))]
pub async fn sys_validation_workflow(
    mut workspace: SysValidationWorkspace<'_>,
    writer: OneshotWriter,
    trigger_app_validation: &mut TriggerSender,
    network: HolochainP2pCell,
) -> WorkflowResult<WorkComplete> {
    let complete = sys_validation_workflow_inner(&mut workspace, network).await?;

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
    workspace: &mut SysValidationWorkspace<'_>,
    network: HolochainP2pCell,
) -> WorkflowResult<WorkComplete> {
    // Drain all the ops
    let mut ops: Vec<ValidationLimboValue> = workspace
        .validation_limbo
        .drain_iter()?
        .filter(|vlv| {
            match vlv.status {
                // We only want pending or awaiting sys dependency ops
                ValidationLimboStatus::Pending | ValidationLimboStatus::AwaitingSysDeps => Ok(true),
                ValidationLimboStatus::SysValidated | ValidationLimboStatus::AwaitingAppDeps => {
                    Ok(false)
                }
            }
        })
        .collect()?;

    // Sort the ops
    ops.sort_unstable_by_key(|v| DhtOpOrder::from(&v.op));

    for vlv in ops {
        let ValidationLimboValue {
            op,
            basis,
            time_added,
            num_tries,
            ..
        } = vlv;
        let (status, op) = validate_op(op, workspace, network.clone()).await?;
        match &status {
            ValidationLimboStatus::Pending
            | ValidationLimboStatus::AwaitingSysDeps
            | ValidationLimboStatus::SysValidated => {
                // TODO: Some of the ops go straight to integration and
                // skip app validation so we need to write those to the
                // integration limbo and not the validation limbo
                let hash = DhtOpHash::with_data(&op).await;
                let vlv = ValidationLimboValue {
                    status,
                    op,
                    basis,
                    time_added,
                    last_try: Some(Timestamp::now()),
                    num_tries: num_tries + 1,
                };
                workspace.validation_limbo.put(hash, vlv)?;
            }
            ValidationLimboStatus::AwaitingAppDeps => {
                unreachable!("We should not be returning this status from system validation")
            }
        }
    }
    Ok(WorkComplete::Complete)
}

async fn validate_op(
    op: DhtOp,
    workspace: &mut SysValidationWorkspace<'_>,
    network: HolochainP2pCell,
) -> WorkflowResult<(ValidationLimboStatus, DhtOp)> {
    match validate_op_inner(op, workspace, network).await {
        Ok(op) => Ok((ValidationLimboStatus::SysValidated, op)),
        // TODO: Handle the errors that result in pending or awaiting deps
        Err(_) => todo!(),
    }
}

async fn validate_op_inner(
    op: DhtOp,
    workspace: &mut SysValidationWorkspace<'_>,
    network: HolochainP2pCell,
) -> SysValidationResult<DhtOp> {
    match op {
        DhtOp::StoreElement(signature, header, maybe_entry) => {
            all_op_check(&signature, &header).await?;
            store_header(&header, workspace.cascade(network)).await?;
            Ok(DhtOp::StoreElement(signature, header, maybe_entry))
        }
        DhtOp::StoreEntry(signature, header, maybe_entry) => {
            let header = header.into();
            all_op_check(&signature, &header).await?;
            Ok(DhtOp::StoreEntry(
                signature,
                header.try_into().expect("type hasn't changed"),
                maybe_entry,
            ))
        }
        DhtOp::RegisterAgentActivity(signature, header) => {
            all_op_check(&signature, &header).await?;
            register_agent_activity(&header, &workspace).await?;
            Ok(DhtOp::RegisterAgentActivity(signature, header))
        }
        DhtOp::RegisterUpdatedBy(signature, header) => {
            let header = header.into();
            all_op_check(&signature, &header).await?;
            Ok(DhtOp::RegisterUpdatedBy(
                signature,
                header.try_into().expect("type hasn't changed"),
            ))
        }
        DhtOp::RegisterDeletedBy(signature, header) => {
            let header = header.into();
            all_op_check(&signature, &header).await?;
            Ok(DhtOp::RegisterDeletedBy(
                signature,
                header.try_into().expect("type hasn't changed"),
            ))
        }
        DhtOp::RegisterDeletedEntryHeader(signature, header) => {
            let header = header.into();
            all_op_check(&signature, &header).await?;
            Ok(DhtOp::RegisterDeletedEntryHeader(
                signature,
                header.try_into().expect("type hasn't changed"),
            ))
        }
        DhtOp::RegisterAddLink(signature, header) => {
            let header = header.into();
            all_op_check(&signature, &header).await?;
            Ok(DhtOp::RegisterAddLink(
                signature,
                header.try_into().expect("type hasn't changed"),
            ))
        }
        DhtOp::RegisterRemoveLink(signature, header) => {
            let header = header.into();
            all_op_check(&signature, &header).await?;
            Ok(DhtOp::RegisterRemoveLink(
                signature,
                header.try_into().expect("type hasn't changed"),
            ))
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
    workspace: &SysValidationWorkspace<'_>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let author = header.author();
    let prev_header_hash = header.prev_header();

    // Checks
    check_prev_header(&header)?;
    check_valid_if_dna(&header, &workspace.meta_vault)?;
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

async fn store_header(header: &Header, cascade: Cascade<'_, '_>) -> SysValidationResult<()> {
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

/// Type for deriving ordering of DhtOps
/// Don't change the order of this enum unless
/// you mean to change the order we process ops
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DhtOpOrder {
    RegisterAgentActivity,
    StoreEntry,
    StoreElement,
    RegisterUpdatedBy,
    RegisterDeletedBy,
    RegisterDeletedEntryHeader,
    RegisterAddLink,
    RegisterRemoveLink,
}

impl From<&DhtOp> for DhtOpOrder {
    fn from(op: &DhtOp) -> Self {
        use DhtOpOrder::*;
        match op {
            DhtOp::StoreElement(_, _, _) => StoreElement,
            DhtOp::StoreEntry(_, _, _) => StoreEntry,
            DhtOp::RegisterAgentActivity(_, _) => RegisterAgentActivity,
            DhtOp::RegisterUpdatedBy(_, _) => RegisterUpdatedBy,
            DhtOp::RegisterDeletedBy(_, _) => RegisterDeletedBy,
            DhtOp::RegisterDeletedEntryHeader(_, _) => RegisterDeletedEntryHeader,
            DhtOp::RegisterAddLink(_, _) => RegisterAddLink,
            DhtOp::RegisterRemoveLink(_, _) => RegisterRemoveLink,
        }
    }
}

pub struct SysValidationWorkspace<'env> {
    pub integration_limbo: IntegrationLimboStore<'env>,
    pub integrated_dht_ops: IntegratedDhtOpsStore<'env>,
    pub validation_limbo: ValidationLimboStore<'env>,
    pub element_vault: ElementBuf<'env>,
    pub meta_vault: MetadataBuf<'env>,
    pub element_cache: ElementBuf<'env>,
    pub meta_cache: MetadataBuf<'env>,
}

impl<'env: 'a, 'a> SysValidationWorkspace<'env> {
    pub fn cascade(&'a mut self, network: HolochainP2pCell) -> Cascade<'env, 'a> {
        Cascade::new(
            &self.element_vault,
            &self.meta_vault,
            &mut self.element_cache,
            &mut self.meta_cache,
            network,
        )
    }
}

impl<'env> Workspace<'env> for SysValidationWorkspace<'env> {
    fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let db = dbs.get_db(&*INTEGRATED_DHT_OPS)?;
        let integrated_dht_ops = KvBuf::new(reader, db)?;

        let db = dbs.get_db(&*INTEGRATION_LIMBO)?;
        let integration_limbo = KvBuf::new(reader, db)?;

        let validation_limbo = ValidationLimboStore::new(reader, dbs)?;

        let element_vault = ElementBuf::vault(reader, dbs, false)?;
        let meta_vault = MetadataBuf::vault(reader, dbs)?;
        let element_cache = ElementBuf::cache(reader, dbs)?;
        let meta_cache = MetadataBuf::cache(reader, dbs)?;

        Ok(Self {
            integration_limbo,
            integrated_dht_ops,
            validation_limbo,
            element_vault,
            meta_vault,
            element_cache,
            meta_cache,
        })
    }
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.validation_limbo.0.flush_to_txn(writer)?;
        self.integration_limbo.flush_to_txn(writer)?;
        // Flush for cascade
        self.element_cache.flush_to_txn(writer)?;
        self.meta_cache.flush_to_txn(writer)?;
        Ok(())
    }
}
