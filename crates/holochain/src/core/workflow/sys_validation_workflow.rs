//! The workflow and queue consumer for sys validation
#![allow(deprecated)]

use super::*;
use crate::conductor::handle::ConductorHandleT;
use crate::conductor::space::Space;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::ribosome::RibosomeT;
use crate::core::sys_validate::check_and_hold_store_element;
use crate::core::sys_validate::*;
use crate::core::validation::*;
use error::WorkflowResult;
use holo_hash::DhtOpHash;
use holochain_cascade::Cascade;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::prelude::*;
use holochain_state::host_fn_workspace::HostFnStores;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::prelude::*;
use holochain_state::scratch::SyncScratch;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::prelude::*;
use holochain_zome_types::Entry;
use holochain_zome_types::ValidationStatus;
use rusqlite::Transaction;
use std::convert::TryInto;
use std::sync::Arc;
use tracing::*;
use types::Outcome;

pub mod types;

pub mod validation_query;

const NUM_CONCURRENT_OPS: usize = 50;

#[cfg(test)]
mod chain_test;
#[cfg(test)]
mod test_ideas;
#[cfg(test)]
mod tests;

#[instrument(skip(
    workspace,
    space,
    trigger_app_validation,
    sys_validation_trigger,
    network,
    conductor_handle
))]
pub async fn sys_validation_workflow(
    workspace: Arc<SysValidationWorkspace>,
    space: Arc<Space>,
    trigger_app_validation: TriggerSender,
    sys_validation_trigger: TriggerSender,
    network: HolochainP2pDna,
    conductor_handle: ConductorHandle,
) -> WorkflowResult<WorkComplete> {
    let complete = sys_validation_workflow_inner(
        workspace,
        space,
        network,
        conductor_handle,
        sys_validation_trigger,
    )
    .await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // trigger other workflows
    trigger_app_validation.trigger(&"sys_validation_workflow");

    Ok(complete)
}

async fn sys_validation_workflow_inner(
    workspace: Arc<SysValidationWorkspace>,
    space: Arc<Space>,
    network: HolochainP2pDna,
    conductor_handle: ConductorHandle,
    sys_validation_trigger: TriggerSender,
) -> WorkflowResult<WorkComplete> {
    let db = workspace.dht_db.clone();
    let sorted_ops = validation_query::get_ops_to_sys_validate(&db).await?;
    let start_len = sorted_ops.len();
    tracing::debug!("Validating {} ops", start_len);
    let start = (start_len >= NUM_CONCURRENT_OPS).then(std::time::Instant::now);
    let saturated = start.is_some();

    // Process each op
    let iter = sorted_ops.into_iter().map({
        let space = space.clone();
        move |so| {
            // Create an incoming ops sender for any dependencies we find
            // that we are meant to be holding but aren't.
            // If we are not holding them they will be added to our incoming ops.
            let incoming_dht_ops_sender =
                IncomingDhtOpSender::new(space.clone(), sys_validation_trigger.clone());
            let network = network.clone();
            let workspace = workspace.clone();
            let conductor_handle = conductor_handle.clone();
            async move {
                let (op, op_hash) = so.into_inner();
                let op_type = op.get_type();
                let header = op.header();

                let dependency = get_dependency(op_type, &header);

                let r = validate_op(
                    &op,
                    &(*workspace),
                    network,
                    conductor_handle.as_ref(),
                    Some(incoming_dht_ops_sender),
                )
                .await;
                r.map(|o| (op_hash, o, dependency))
            }
        }
    });

    // Create a stream of concurrent validation futures.
    // This will run NUM_CONCURRENT_OPS validation futures concurrently and
    // return up to NUM_CONCURRENT_OPS * 100 results.
    use futures::stream::StreamExt;
    let mut iter = futures::stream::iter(iter)
        .buffer_unordered(NUM_CONCURRENT_OPS)
        .ready_chunks(NUM_CONCURRENT_OPS * 100);

    // Spawn a task to actually drive the stream.
    // This allows the stream to make progress in the background while
    // we are committing previous results to the database.
    let (tx, rx) = tokio::sync::mpsc::channel(NUM_CONCURRENT_OPS * 100);
    let jh = tokio::spawn(async move {
        // Send the result to task that will commit to the database.
        while let Some(op) = iter.next().await {
            if tx.send(op).await.is_err() {
                tracing::warn!("app validation task has failed to send ops. This is not a problem if the conductor is shutting down");
                break;
            }
        }
    });

    // Create a stream that will chunk up to NUM_CONCURRENT_OPS * 100 ready results.
    let mut iter =
        tokio_stream::wrappers::ReceiverStream::new(rx).ready_chunks(NUM_CONCURRENT_OPS * 100);

    let mut total = 0;
    let mut round_time = start.is_some().then(std::time::Instant::now);
    // Pull in a chunk of results.
    while let Some(chunk) = iter.next().await {
        let num_ops: usize = chunk.iter().map(|c| c.len()).sum();
        tracing::debug!("Committing {} ops", num_ops);
        let (t, a, m, r) = space
            .dht_db
            .async_commit(move |txn| {
                let mut total = 0;
                let mut awaiting = 0;
                let mut missing = 0;
                let mut rejected = 0;
                for outcome in chunk.into_iter().flatten() {
                    let (op_hash, outcome, dependency) = outcome?;
                    match outcome {
                        Outcome::Accepted => {
                            total += 1;
                            put_validation_limbo(
                                txn,
                                &op_hash,
                                ValidationLimboStatus::SysValidated,
                            )?;
                        }
                        Outcome::AwaitingOpDep(missing_dep) => {
                            awaiting += 1;
                            // TODO: Try and get this dependency to add to limbo
                            //
                            // I actually can't see how we can do this because there's no
                            // way to get an DhtOpHash without either having the op or the full
                            // header. We have neither that's why where here.
                            //
                            // We need to be holding the dependency because
                            // we were meant to get a StoreElement or StoreEntry or
                            // RegisterAgentActivity or RegisterAddLink.
                            let status = ValidationLimboStatus::AwaitingSysDeps(missing_dep);
                            put_validation_limbo(txn, &op_hash, status)?;
                        }
                        Outcome::MissingDhtDep => {
                            missing += 1;
                            // TODO: Not sure what missing dht dep is. Check if we need this.
                            put_validation_limbo(txn, &op_hash, ValidationLimboStatus::Pending)?;
                        }
                        Outcome::Rejected => {
                            rejected += 1;
                            if let Dependency::Null = dependency {
                                put_integrated(txn, &op_hash, ValidationStatus::Rejected)?;
                            } else {
                                put_integration_limbo(txn, &op_hash, ValidationStatus::Rejected)?;
                            }
                        }
                    }
                }
                WorkflowResult::Ok((total, awaiting, missing, rejected))
            })
            .await?;

        total += t;
        if let (Some(start), Some(round_time)) = (start, &mut round_time) {
            let round_el = round_time.elapsed();
            *round_time = std::time::Instant::now();
            let avg_ops_ps = total as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
            let ops_ps = t as f64 / round_el.as_micros() as f64 * 1_000_000.0;
            tracing::info!(
                "Sys validation is saturated. Util {:.2}%. OPS/s avg {:.2}, this round {:.2}",
                (start_len - total) as f64 / NUM_CONCURRENT_OPS as f64 * 100.0,
                avg_ops_ps,
                ops_ps
            );
        }
        tracing::debug!("{} committed, {} awaiting sys dep, {} missing dht dep, {} rejected. {} committed this round", t, a, m, r, total);
    }
    jh.await?;
    tracing::debug!("Accepted {} ops", total);
    Ok(if saturated {
        WorkComplete::Incomplete
    } else {
        WorkComplete::Complete
    })
}

async fn validate_op(
    op: &DhtOp,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    conductor_handle: &dyn ConductorHandleT,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
) -> WorkflowResult<Outcome> {
    match validate_op_inner(
        op,
        workspace,
        network,
        conductor_handle,
        incoming_dht_ops_sender,
    )
    .await
    {
        Ok(_) => Ok(Outcome::Accepted),
        // Handle the errors that result in pending or awaiting deps
        Err(SysValidationError::ValidationOutcome(e)) => {
            info!(
                dna = %workspace.dna_hash(),
                msg = "DhtOp did not pass system validation. (If rejected, a warning will follow.)",
                ?op,
                error = ?e,
                error_msg = %e
            );
            let outcome = handle_failed(e);
            if let Outcome::Rejected = outcome {
                warn!(
                    dna = %workspace.dna_hash(),
                    msg = "DhtOp was rejected during system validation.",
                    ?op,
                )
            }
            Ok(outcome)
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
        ValidationOutcome::Counterfeit(_, _) => {
            unreachable!("Counterfeit ops are dropped before sys validation")
        }
        ValidationOutcome::HeaderNotInCounterSigningSession(_, _) => Rejected,
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
        ValidationOutcome::PreflightResponseSignature(_) => Rejected,
        ValidationOutcome::UpdateTypeMismatch(_, _) => Rejected,
        ValidationOutcome::VerifySignature(_, _) => Rejected,
        ValidationOutcome::ZomeId(_) => Rejected,
        ValidationOutcome::CounterSigningError(_) => Rejected,
    }
}

async fn validate_op_inner(
    op: &DhtOp,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    conductor_handle: &dyn ConductorHandleT,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
) -> SysValidationResult<()> {
    let dna_hash = network.dna_hash();
    let ribosome = conductor_handle
        .get_ribosome(dna_hash)
        .map_err(|_| SysValidationError::DnaMissing(dna_hash.clone()))?;
    match op {
        DhtOp::StoreElement(_, header, entry) => {
            store_element(header, workspace, network.clone()).await?;
            if let Some(entry) = entry {
                // Retrieve for all other headers on countersigned entry.
                if let Entry::CounterSign(session_data, _) = &**entry {
                    let entry_hash = EntryHash::with_data_sync(&**entry);
                    for header in session_data.build_header_set(entry_hash)? {
                        let header = if let Some(zome_id) = header.zome_id() {
                            let zome = workspace.dna_def.get_zome_by_index(&zome_id)?;
                            ribosome.weigh_countersigning_header(header, (**entry).clone(), zome)?
                        } else {
                            header.weighed(Default::default())
                        };
                        let hh = HeaderHash::with_data_sync(&header);
                        if workspace
                            .full_cascade(network.clone())
                            .retrieve_header(hh.clone(), Default::default())
                            .await?
                            .is_none()
                        {
                            return Err(SysValidationError::ValidationOutcome(
                                ValidationOutcome::DepMissingFromDht(hh.into()),
                            ));
                        }
                    }
                }
                store_entry(
                    (header)
                        .try_into()
                        .map_err(|_| ValidationOutcome::NotNewEntry(header.clone()))?,
                    entry.as_ref(),
                    conductor_handle,
                    workspace,
                    network,
                )
                .await?;
            }
            Ok(())
        }
        DhtOp::StoreEntry(_, header, entry) => {
            // Check and hold for all other headers on countersigned entry.
            if let Entry::CounterSign(session_data, _) = &**entry {
                let dependency_check = |_original_element: &Element| Ok(());
                let entry_hash = EntryHash::with_data_sync(&**entry);
                for header in session_data.build_header_set(entry_hash)? {
                    let header = if let Some(zome_id) = header.zome_id() {
                        let zome = workspace.dna_def.get_zome_by_index(&zome_id)?;
                        ribosome.weigh_countersigning_header(header, (**entry).clone(), zome)?
                    } else {
                        header.weighed(Default::default())
                    };
                    check_and_hold_store_element(
                        &HeaderHash::with_data_sync(&header),
                        workspace,
                        network.clone(),
                        incoming_dht_ops_sender.clone(),
                        dependency_check,
                    )
                    .await?;
                }
            }

            store_entry(
                (header).into(),
                entry.as_ref(),
                conductor_handle,
                workspace,
                network.clone(),
            )
            .await?;

            let header = header.clone().into();
            store_element(&header, workspace, network).await?;
            Ok(())
        }
        DhtOp::RegisterAgentActivity(_, header) => {
            register_agent_activity(header, workspace, network.clone(), incoming_dht_ops_sender)
                .await?;
            store_element(header, workspace, network).await?;
            Ok(())
        }
        DhtOp::RegisterUpdatedContent(_, header, entry) => {
            register_updated_content(header, workspace, network.clone(), incoming_dht_ops_sender)
                .await?;
            if let Some(entry) = entry {
                store_entry(
                    NewEntryHeaderRef::Update(header),
                    entry.as_ref(),
                    conductor_handle,
                    workspace,
                    network.clone(),
                )
                .await?;
            }

            Ok(())
        }
        DhtOp::RegisterUpdatedElement(_, header, entry) => {
            register_updated_element(header, workspace, network.clone(), incoming_dht_ops_sender)
                .await?;
            if let Some(entry) = entry {
                store_entry(
                    NewEntryHeaderRef::Update(header),
                    entry.as_ref(),
                    conductor_handle,
                    workspace,
                    network.clone(),
                )
                .await?;
            }

            Ok(())
        }
        DhtOp::RegisterDeletedBy(_, header) => {
            register_deleted_by(header, workspace, network, incoming_dht_ops_sender).await?;
            Ok(())
        }
        DhtOp::RegisterDeletedEntryHeader(_, header) => {
            register_deleted_entry_header(header, workspace, network, incoming_dht_ops_sender)
                .await?;
            Ok(())
        }
        DhtOp::RegisterAddLink(_, header) => {
            register_add_link(header, workspace, network, incoming_dht_ops_sender).await?;
            Ok(())
        }
        DhtOp::RegisterRemoveLink(_, header) => {
            register_delete_link(header, workspace, network, incoming_dht_ops_sender).await?;
            Ok(())
        }
    }
}

#[instrument(skip(element, call_zome_workspace, network, conductor_handle))]
/// Direct system validation call that takes
/// an Element instead of an op.
/// Does not require holding dependencies.
/// Will not await dependencies and instead returns
/// that outcome immediately.
pub async fn sys_validate_element(
    element: &Element,
    call_zome_workspace: &HostFnWorkspace,
    network: HolochainP2pDna,
    conductor_handle: &dyn ConductorHandleT,
) -> SysValidationOutcome<()> {
    trace!(?element);
    // Create a SysValidationWorkspace with the scratches from the CallZomeWorkspace
    let workspace = SysValidationWorkspace::from(call_zome_workspace);
    let result =
        match sys_validate_element_inner(element, &workspace, network, conductor_handle).await {
            // Validation succeeded
            Ok(_) => Ok(()),
            // Validation failed so exit with that outcome
            Err(SysValidationError::ValidationOutcome(validation_outcome)) => {
                error!(msg = "Direct validation failed", ?element);
                validation_outcome.into_outcome()
            }
            // An error occurred so return it
            Err(e) => Err(OutcomeOrError::Err(e)),
        };

    result
}

async fn sys_validate_element_inner(
    element: &Element,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    conductor_handle: &dyn ConductorHandleT,
) -> SysValidationResult<()> {
    let signature = element.signature();
    let header = element.header();
    let maybe_entry = element.entry().as_option();
    counterfeit_check(signature, header).await?;

    async fn validate(
        header: &Header,
        maybe_entry: Option<&Entry>,
        workspace: &SysValidationWorkspace,
        network: HolochainP2pDna,
        conductor_handle: &dyn ConductorHandleT,
    ) -> SysValidationResult<()> {
        let incoming_dht_ops_sender = None;
        store_element(header, workspace, network.clone()).await?;
        if let Some((maybe_entry, EntryVisibility::Public)) =
            &maybe_entry.and_then(|e| header.entry_type().map(|et| (e, et.visibility())))
        {
            store_entry(
                (header)
                    .try_into()
                    .map_err(|_| ValidationOutcome::NotNewEntry(header.clone()))?,
                maybe_entry,
                conductor_handle,
                workspace,
                network.clone(),
            )
            .await?;
        }
        match header {
            Header::Update(header) => {
                register_updated_content(header, workspace, network, incoming_dht_ops_sender)
                    .await?;
            }
            Header::Delete(header) => {
                register_deleted_entry_header(header, workspace, network, incoming_dht_ops_sender)
                    .await?;
            }
            Header::CreateLink(header) => {
                register_add_link(header, workspace, network, incoming_dht_ops_sender).await?;
            }
            Header::DeleteLink(header) => {
                register_delete_link(header, workspace, network, incoming_dht_ops_sender).await?;
            }
            _ => {}
        }
        Ok(())
    }

    match maybe_entry {
        Some(Entry::CounterSign(session, _)) => {
            let entry = maybe_entry.unwrap();
            let entry_hash = EntryHash::with_data_sync(entry);
            let dna_hash = network.dna_hash();
            let ribosome = conductor_handle
                .get_ribosome(dna_hash)
                .map_err(|_| SysValidationError::DnaMissing(dna_hash.clone()))?;
            for header in session.build_header_set(entry_hash)? {
                let header = if let Some(zome_id) = header.zome_id() {
                    let zome = workspace.dna_def.get_zome_by_index(&zome_id)?;
                    ribosome.weigh_countersigning_header(header, entry.clone(), zome)?
                } else {
                    header.weighed(Default::default())
                };
                validate(
                    &header.into(),
                    maybe_entry,
                    workspace,
                    network.clone(),
                    conductor_handle,
                )
                .await?;
            }
            Ok(())
        }
        _ => validate(header, maybe_entry, workspace, network, conductor_handle).await,
    }
}

/// Check if the op has valid signature and author.
/// Ops that fail this check should be dropped.
pub async fn counterfeit_check(signature: &Signature, header: &Header) -> SysValidationResult<()> {
    verify_header_signature(signature, header).await?;
    author_key_is_valid(header.author()).await?;
    Ok(())
}

async fn register_agent_activity(
    header: &Header,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let prev_header_hash = header.prev_header();

    // Checks
    check_prev_header(header)?;
    check_valid_if_dna(header, workspace).await?;
    if let Some(prev_header_hash) = prev_header_hash {
        check_and_hold_register_agent_activity(
            prev_header_hash,
            workspace,
            network,
            incoming_dht_ops_sender,
            |_| Ok(()),
        )
        .await?;
    }
    check_chain_rollback(header, workspace).await?;
    Ok(())
}

async fn store_element(
    header: &Header,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let prev_header_hash = header.prev_header();

    // Checks
    check_prev_header(header)?;
    if let Some(prev_header_hash) = prev_header_hash {
        let mut cascade = workspace.full_cascade(network);
        let prev_header = cascade
            .retrieve_header(prev_header_hash.clone(), Default::default())
            .await?
            .ok_or_else(|| ValidationOutcome::DepMissingFromDht(prev_header_hash.clone().into()))?;
        check_prev_timestamp(header, prev_header.header())?;
        check_prev_seq(header, prev_header.header())?;
    }
    Ok(())
}

async fn store_entry(
    header: NewEntryHeaderRef<'_>,
    entry: &Entry,
    conductor_handle: &dyn ConductorHandleT,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let entry_type = header.entry_type();
    let entry_hash = header.entry_hash();

    // Checks
    check_entry_type(entry_type, entry)?;
    if let EntryType::App(app_entry_type) = entry_type {
        let entry_def =
            check_app_entry_type(workspace.dna_hash(), app_entry_type, conductor_handle).await?;
        check_not_private(&entry_def)?;
    }

    check_entry_hash(entry_hash, entry).await?;
    check_entry_size(entry)?;

    // Additional checks if this is an Update
    if let NewEntryHeaderRef::Update(entry_update) = header {
        let original_header_address = &entry_update.original_header_address;
        let mut cascade = workspace.full_cascade(network);
        let original_header = cascade
            .retrieve_header(original_header_address.clone(), Default::default())
            .await?
            .ok_or_else(|| {
                ValidationOutcome::DepMissingFromDht(original_header_address.clone().into())
            })?;
        update_check(entry_update, original_header.header())?;
    }

    // Additional checks if this is a countersigned entry.
    if let Entry::CounterSign(session_data, _) = entry {
        check_countersigning_session_data(EntryHash::with_data_sync(entry), session_data, header)
            .await?;
    }
    Ok(())
}

async fn register_updated_content(
    entry_update: &Update,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_header_address = &entry_update.original_header_address;

    let dependency_check =
        |original_element: &Element| update_check(entry_update, original_element.header());
    check_and_hold_store_entry(
        original_header_address,
        workspace,
        network,
        incoming_dht_ops_sender,
        dependency_check,
    )
    .await?;
    Ok(())
}

async fn register_updated_element(
    entry_update: &Update,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_header_address = &entry_update.original_header_address;

    let dependency_check =
        |original_element: &Element| update_check(entry_update, original_element.header());

    check_and_hold_store_element(
        original_header_address,
        workspace,
        network,
        incoming_dht_ops_sender,
        dependency_check,
    )
    .await?;
    Ok(())
}

async fn register_deleted_by(
    element_delete: &Delete,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_header_address = &element_delete.deletes_address;

    // Checks
    let dependency_check =
        |removed_header: &Element| check_new_entry_header(removed_header.header());

    check_and_hold_store_element(
        removed_header_address,
        workspace,
        network,
        incoming_dht_ops_sender,
        dependency_check,
    )
    .await?;
    Ok(())
}

async fn register_deleted_entry_header(
    element_delete: &Delete,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_header_address = &element_delete.deletes_address;

    // Checks
    let dependency_check =
        |removed_header: &Element| check_new_entry_header(removed_header.header());

    check_and_hold_store_entry(
        removed_header_address,
        workspace,
        network,
        incoming_dht_ops_sender,
        dependency_check,
    )
    .await?;
    Ok(())
}

async fn register_add_link(
    link_add: &CreateLink,
    _workspace: &SysValidationWorkspace,
    _network: HolochainP2pDna,
    _incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
) -> SysValidationResult<()> {
    check_tag_size(&link_add.tag)?;
    Ok(())
}

async fn register_delete_link(
    link_remove: &DeleteLink,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let link_add_address = &link_remove.link_add_address;

    // Checks
    check_and_hold_register_add_link(
        link_add_address,
        workspace,
        network,
        incoming_dht_ops_sender,
        |_| Ok(()),
    )
    .await?;
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
    scratch: Option<SyncScratch>,
    authored_db: DbRead<DbKindAuthored>,
    dht_db: DbRead<DbKindDht>,
    dht_query_cache: Option<DhtDbQueryCache>,
    cache: DbWrite<DbKindCache>,
    pub(crate) dna_def: Arc<DnaDef>,
}

impl SysValidationWorkspace {
    pub fn new(
        authored_db: DbRead<DbKindAuthored>,
        dht_db: DbRead<DbKindDht>,
        dht_query_cache: DhtDbQueryCache,
        cache: DbWrite<DbKindCache>,
        dna_def: Arc<DnaDef>,
    ) -> Self {
        Self {
            authored_db,
            dht_db,
            dht_query_cache: Some(dht_query_cache),
            cache,
            dna_def,
            scratch: None,
        }
    }

    pub async fn is_chain_empty(&self, author: &AgentPubKey) -> SourceChainResult<bool> {
        // If we have a query cache then this is an authority node and
        // we can quickly check if the chain is empty from the cache.
        if let Some(c) = &self.dht_query_cache {
            return Ok(c.is_chain_empty(author).await?);
        }

        // Otherwise we need to check this is an author node and
        // we need to check the author db.
        let author = author.clone();
        let chain_not_empty = self
            .authored_db
            .async_reader(move |txn| {
                let mut stmt = txn.prepare(
                    "
                SELECT
                EXISTS (
                    SELECT
                    1
                    FROM Header
                    JOIN
                    DhtOp ON Header.hash = DhtOp.header_hash
                    WHERE
                    Header.author = :author
                    AND
                    DhtOp.when_integrated IS NOT NULL
                    AND
                    DhtOp.type = :activity
                    LIMIT 1
                )
                ",
                )?;
                DatabaseResult::Ok(stmt.query_row(
                    named_params! {
                        ":author": author,
                        ":activity": DhtOpType::RegisterAgentActivity,
                    },
                    |row| row.get(0),
                )?)
            })
            .await?;
        let chain_not_empty = match &self.scratch {
            Some(scratch) => scratch.apply(|scratch| !scratch.is_empty())? || chain_not_empty,
            None => chain_not_empty,
        };
        Ok(!chain_not_empty)
    }

    pub async fn header_seq_is_empty(&self, header: &Header) -> SourceChainResult<bool> {
        let author = header.author().clone();
        let seq = header.header_seq();
        let hash = HeaderHash::with_data_sync(header);
        let header_seq_is_not_empty = self
            .dht_db
            .async_reader({
                let hash = hash.clone();
                move |txn| {
                    DatabaseResult::Ok(txn.query_row(
                        "
                SELECT EXISTS(
                    SELECT
                    1
                    FROM Header
                    WHERE
                    Header.author = :author
                    AND
                    Header.seq = :seq
                    AND
                    Header.hash != :hash
                    LIMIT 1
                )
                ",
                        named_params! {
                            ":author": author,
                            ":seq": seq,
                            ":hash": hash,
                        },
                        |row| row.get(0),
                    )?)
                }
            })
            .await?;
        let header_seq_is_not_empty = match &self.scratch {
            Some(scratch) => {
                scratch.apply(|scratch| {
                    scratch.headers().any(|shh| {
                        shh.header().header_seq() == seq && *shh.header_address() != hash
                    })
                })? || header_seq_is_not_empty
            }
            None => header_seq_is_not_empty,
        };
        Ok(!header_seq_is_not_empty)
    }
    /// Create a cascade with local data only
    pub fn local_cascade(&self) -> Cascade {
        let cascade = Cascade::empty().with_dht(self.dht_db.clone());
        match &self.scratch {
            Some(scratch) => cascade
                .with_authored(self.authored_db.clone())
                .with_scratch(scratch.clone()),
            None => cascade,
        }
    }
    pub fn full_cascade<Network: HolochainP2pDnaT + Clone + 'static + Send>(
        &self,
        network: Network,
    ) -> Cascade<Network> {
        let cascade = Cascade::empty()
            .with_dht(self.dht_db.clone())
            .with_network(network, self.cache.clone());
        match &self.scratch {
            Some(scratch) => cascade
                .with_authored(self.authored_db.clone())
                .with_scratch(scratch.clone()),
            None => cascade,
        }
    }

    fn dna_hash(&self) -> &DnaHash {
        self.dht_db.kind().dna_hash()
    }

    /// Get a reference to the sys validation workspace's dna def.
    pub fn dna_def(&self) -> Arc<DnaDef> {
        self.dna_def.clone()
    }
}

fn put_validation_limbo(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    status: ValidationLimboStatus,
) -> WorkflowResult<()> {
    set_validation_stage(txn, hash, status)?;
    Ok(())
}

fn put_integration_limbo(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    status: ValidationStatus,
) -> WorkflowResult<()> {
    set_validation_status(txn, hash, status)?;
    set_validation_stage(txn, hash, ValidationLimboStatus::AwaitingIntegration)?;
    Ok(())
}

pub fn put_integrated(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    status: ValidationStatus,
) -> WorkflowResult<()> {
    set_validation_status(txn, hash, status)?;
    // This set the validation stage to pending which is correct when
    // it's integrated.
    set_validation_stage(txn, hash, ValidationLimboStatus::Pending)?;
    set_when_integrated(txn, hash, Timestamp::now())?;
    Ok(())
}

impl From<&HostFnWorkspace> for SysValidationWorkspace {
    fn from(h: &HostFnWorkspace) -> Self {
        let HostFnStores {
            cache,
            scratch,
            authored,
            dht,
        } = h.stores();
        Self {
            scratch,
            authored_db: authored,
            dht_db: dht,
            dht_query_cache: None,
            cache,
            dna_def: h.dna_def(),
        }
    }
}
