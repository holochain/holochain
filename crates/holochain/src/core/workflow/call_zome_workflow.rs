use super::app_validation_workflow;
use super::app_validation_workflow::AppValidationError;
use super::app_validation_workflow::Outcome;
use super::error::WorkflowResult;
use super::sys_validation_workflow::sys_validate_record;
use crate::conductor::api::CellConductorApi;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::api::DpkiApi;
use crate::conductor::ConductorHandle;
use crate::core::check_dpki_agent_validity_for_record;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::post_commit::send_post_commit;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCallHostAccess;
use crate::core::ribosome::ZomeCallInvocation;
use crate::core::workflow::WorkflowError;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDna;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_state::prelude::IncompleteCommitReason;
use holochain_state::source_chain::SourceChainError;
use holochain_types::prelude::*;
use holochain_zome_types::record::Record;
use std::sync::Arc;
use tokio::sync::broadcast;

#[cfg(test)]
mod validation_test;

/// Placeholder for the return value of a zome invocation
pub type ZomeCallResult = RibosomeResult<ZomeCallResponse>;

pub struct CallZomeWorkflowArgs<RibosomeT> {
    pub ribosome: RibosomeT,
    pub invocation: ZomeCallInvocation,
    pub signal_tx: broadcast::Sender<Signal>,
    pub conductor_handle: ConductorHandle,
    pub is_root_zome_call: bool,
    pub cell_id: CellId,
}

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(
        workspace,
        network,
        keystore,
        args,
        trigger_publish_dht_ops,
        trigger_integrate_dht_ops
    ))
)]
pub async fn call_zome_workflow<Ribosome>(
    workspace: SourceChainWorkspace,
    network: HolochainP2pDna,
    keystore: MetaLairClient,
    args: CallZomeWorkflowArgs<Ribosome>,
    trigger_publish_dht_ops: TriggerSender,
    trigger_integrate_dht_ops: TriggerSender,
) -> WorkflowResult<ZomeCallResult>
where
    Ribosome: RibosomeT + 'static,
{
    let coordinator_zome = args
        .ribosome
        .dna_def()
        .get_coordinator_zome(args.invocation.zome.zome_name())
        .or_else(|_| {
            args.ribosome
                .dna_def()
                .get_integrity_zome(args.invocation.zome.zome_name())
                .map(CoordinatorZome::from)
        })
        .ok();
    let should_write = args.is_root_zome_call;
    let conductor_handle = args.conductor_handle.clone();
    let maybe_dpki = args.conductor_handle.running_services().dpki;
    let signal_tx = args.signal_tx.clone();
    let result = call_zome_workflow_inner(
        workspace.clone(),
        maybe_dpki,
        network.clone(),
        keystore.clone(),
        args,
    )
    .await?;
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    if should_write {
        let countersigning_op = workspace.source_chain().countersigning_op()?;
        match workspace.source_chain().flush(&network).await {
            Ok(flushed_actions) => {
                // Skip if nothing was written
                if !flushed_actions.is_empty() {
                    match countersigning_op {
                        Some(op) => {
                            if let Err(error_response) =
                                super::countersigning_workflow::countersigning_publish(
                                    &network,
                                    op,
                                    (*workspace.author().ok_or_else(|| {
                                        WorkflowError::Other("author required".into())
                                    })?)
                                    .clone(),
                                )
                                .await
                            {
                                return Ok(Ok(error_response));
                            }
                        }
                        None => {
                            trigger_publish_dht_ops.trigger(&"call_zome_workflow");
                            trigger_integrate_dht_ops.trigger(&"call_zome_workflow");
                        }
                    }

                    // Only send post commit if this is a coordinator zome.
                    if let Some(coordinator_zome) = coordinator_zome {
                        send_post_commit(
                            conductor_handle,
                            workspace,
                            network,
                            keystore,
                            flushed_actions,
                            vec![coordinator_zome],
                            signal_tx,
                        )
                        .await?;
                    }
                }
            }
            err => {
                err?;
            }
        }
    };
    Ok(result)
}

async fn call_zome_workflow_inner<Ribosome>(
    workspace: SourceChainWorkspace,
    dpki: DpkiApi,
    network: HolochainP2pDna,
    keystore: MetaLairClient,
    args: CallZomeWorkflowArgs<Ribosome>,
) -> WorkflowResult<ZomeCallResult>
where
    Ribosome: RibosomeT + 'static,
{
    let CallZomeWorkflowArgs {
        ribosome,
        invocation,
        signal_tx,
        conductor_handle,
        cell_id,
        ..
    } = args;

    let call_zome_handle =
        CellConductorApi::new(conductor_handle.clone(), cell_id).into_call_zome_handle();

    tracing::trace!("Before zome call");
    let host_access = ZomeCallHostAccess::new(
        workspace.clone().into(),
        keystore,
        dpki,
        network.clone(),
        signal_tx,
        call_zome_handle,
    );
    let (ribosome, result) =
        call_zome_function_authorized(ribosome, host_access, invocation).await?;
    tracing::trace!("After zome call");

    let validation_result =
        inline_validation(workspace.clone(), network, conductor_handle, ribosome).await;

    // If the validation failed remove any active chain lock that matches the
    // entry that failed validation.
    // Note that missing dependencies will not produce an `InvalidCommit` but an `IncompleteCommit`
    // so that the commit can be retried later without terminating the countersigning session.
    if matches!(
        validation_result,
        Err(WorkflowError::SourceChainError(
            SourceChainError::InvalidCommit(_)
        ))
    ) {
        let scratch_records = workspace.source_chain().scratch_records()?;
        if scratch_records.len() == 1 {
            let lock_subject = holochain_state::source_chain::chain_lock_subject_for_entry(
                scratch_records[0].entry().as_option(),
            )?;

            // If this wasn't a countersigning commit then the lock will be empty.
            if !lock_subject.is_empty() {
                // Otherwise, we can check whether the chain was locked with a subject matching
                // the entry that failed validation.
                if let Some(subject) = workspace.source_chain().is_chain_locked().await? {
                    // Here we know the chain is locked, and if the lock subject matches the entry
                    // that the app was trying to commit then we can unlock the chain.
                    if subject == lock_subject {
                        if let Err(error) = workspace.source_chain().unlock_chain().await {
                            tracing::error!(?error);
                        }
                    }
                }
            }
        }
    }

    validation_result?;
    Ok(result)
}

/// First check if we are authorized to call
/// the zome function.
/// Then send to a background thread and
/// call the zome function.
pub async fn call_zome_function_authorized<R>(
    ribosome: R,
    host_access: ZomeCallHostAccess,
    invocation: ZomeCallInvocation,
) -> WorkflowResult<(R, RibosomeResult<ZomeCallResponse>)>
where
    R: RibosomeT + 'static,
{
    match invocation.is_authorized(&host_access).await? {
        ZomeCallAuthorization::Authorized => {
            let r = ribosome.call_zome_function(host_access, invocation).await;
            Ok((ribosome, r))
        }
        not_authorized_reason => Ok((
            ribosome,
            Ok(ZomeCallResponse::Unauthorized(
                not_authorized_reason,
                invocation.cell_id.clone(),
                invocation.zome.zome_name().clone(),
                invocation.fn_name.clone(),
                invocation.provenance.clone(),
            )),
        )),
    }
}

/// Run validation inline and wait for the result.
pub async fn inline_validation<Ribosome>(
    workspace: SourceChainWorkspace,
    network: HolochainP2pDna,
    conductor_handle: ConductorHandle,
    ribosome: Ribosome,
) -> WorkflowResult<()>
where
    Ribosome: RibosomeT + 'static,
{
    let cascade = Arc::new(holochain_cascade::CascadeImpl::from_workspace_and_network(
        &workspace,
        Arc::new(network.clone()),
    ));

    let scratch_records = workspace.source_chain().scratch_records()?;

    if let Some(dpki) = conductor_handle.running_services().dpki.clone() {
        // Don't check DPKI validity on DPKI itself!
        if !dpki.is_deepkey_dna(workspace.source_chain().cell_id().dna_hash()) {
            // Check the validity of the author as-at the first and the last record to be committed.
            // If these are valid, then the author is valid for the entire commit.
            let first = scratch_records.first();
            let last = scratch_records.last();
            if let Some(r) = first {
                check_dpki_agent_validity_for_record(&dpki, r).await?;
            }
            if let Some(r) = last {
                if first != last {
                    check_dpki_agent_validity_for_record(&dpki, r).await?;
                }
            }
        }
    }

    let records = {
        // collect all the records we need to validate in wasm
        let mut to_app_validate: Vec<Record> = Vec::with_capacity(scratch_records.len());
        // Loop forwards through all the new records
        for record in scratch_records {
            sys_validate_record(&record, cascade.clone())
                .await
                // If the was en error exit
                // If the validation failed, exit with an InvalidCommit
                // If the validation failed with a retryable error, exit with an IncompleteCommit
                // If it was ok continue
                .or_else(|outcome_or_err| outcome_or_err.into_workflow_error())?;
            to_app_validate.push(record);
        }

        to_app_validate
    };

    let dpki = conductor_handle.running_services().dpki;

    for mut chain_record in records {
        for op_type in action_to_op_types(chain_record.action()) {
            let outcome =
                app_validation_workflow::record_to_op(chain_record, op_type, cascade.clone()).await;

            let (op, _, omitted_entry) = match outcome {
                Ok(op) => op,
                Err(outcome_or_err) => return map_outcome(Outcome::try_from(outcome_or_err)),
            };

            let outcome = app_validation_workflow::validate_op(
                &op,
                workspace.clone().into(),
                &network,
                &ribosome,
                &conductor_handle,
                dpki.clone(),
                true, // is_inline
            )
            .await;
            let outcome = outcome.or_else(Outcome::try_from);
            map_outcome(outcome)?;
            chain_record = op_to_record(op, omitted_entry);
        }
    }

    Ok(())
}

fn op_to_record(op: Op, omitted_entry: Option<Entry>) -> Record {
    match op {
        Op::StoreRecord(StoreRecord { mut record }) => {
            if let Some(e) = omitted_entry {
                // NOTE: this is only possible in this situation because we already removed
                // this exact entry from this Record earlier. DON'T set entries on records
                // anywhere else without recomputing hashes and signatures!
                record.entry = RecordEntry::Present(e);
            }
            record
        }
        Op::StoreEntry(StoreEntry { action, entry }) => {
            Record::new(SignedActionHashed::raw_from_same_hash(action), Some(entry))
        }
        Op::RegisterUpdate(RegisterUpdate {
            update, new_entry, ..
        }) => Record::new(SignedActionHashed::raw_from_same_hash(update), new_entry),
        Op::RegisterDelete(RegisterDelete { delete, .. }) => Record::new(
            SignedActionHashed::raw_from_same_hash(delete),
            omitted_entry,
        ),
        Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => Record::new(
            SignedActionHashed::raw_from_same_hash(action),
            omitted_entry,
        ),
        Op::RegisterCreateLink(RegisterCreateLink { create_link, .. }) => Record::new(
            SignedActionHashed::raw_from_same_hash(create_link),
            omitted_entry,
        ),
        Op::RegisterDeleteLink(RegisterDeleteLink { delete_link, .. }) => Record::new(
            SignedActionHashed::raw_from_same_hash(delete_link),
            omitted_entry,
        ),
    }
}

fn map_outcome(outcome: Result<Outcome, AppValidationError>) -> WorkflowResult<()> {
    match outcome.map_err(SourceChainError::other)? {
        app_validation_workflow::Outcome::Accepted => {}
        app_validation_workflow::Outcome::Rejected(reason) => {
            return Err(SourceChainError::InvalidCommit(format!(
                "Validation failed while committing: {reason}"
            ))
            .into());
        }
        // When the wasm is being called directly in a zome invocation, any state other than valid
        // is not allowed for new entries. E.g. we require that all dependencies are met when
        // committing an entry to a local source chain.
        // This is different to the case where we are validating data coming in from the network
        // where unmet dependencies would be rescheduled to attempt later due to partitions etc.
        // To allow the client to decide whether to retry later, we return a different error
        // variant here. This indicates that the validation did not fail because the data is
        // definitely invalid, but because validation could not make a decision yet.
        Outcome::AwaitingDeps(hashes) => {
            return Err(SourceChainError::IncompleteCommit(
                IncompleteCommitReason::DepMissingFromDht(hashes),
            )
            .into());
        }
    }
    Ok(())
}
