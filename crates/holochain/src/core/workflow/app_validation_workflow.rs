//! The workflow and queue consumer for sys validation
use super::error::WorkflowResult;
use super::sys_validation_workflow::validation_query;

use crate::conductor::entry_def_store::get_entry_def;
use crate::conductor::Conductor;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::ribosome::guest_callback::validate::ValidateHostAccess;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::validation::OutcomeOrError;
use crate::core::SysValidationError;
use crate::core::SysValidationResult;
use crate::core::ValidationOutcome;

pub use error::*;
pub use types::Outcome;
pub use validation_dependencies::ValidationDependencies;

use holo_hash::DhtOpHash;
use holochain_cascade::Cascade;
use holochain_cascade::CascadeImpl;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::GenericNetwork;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_state::prelude::*;
use parking_lot::Mutex;
use rusqlite::Transaction;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tracing::*;

mod validation_dependencies;

#[cfg(todo_redo_old_tests)]
mod network_call_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod validation_tests;

#[cfg(test)]
mod get_zomes_to_invoke_tests;

#[cfg(test)]
mod run_validation_callback_tests;

mod error;
mod types;

#[instrument(skip(
    workspace,
    trigger_integration,
    conductor_handle,
    network,
    dht_query_cache,
    validation_dependencies,
))]
pub async fn app_validation_workflow(
    dna_hash: Arc<DnaHash>,
    workspace: Arc<AppValidationWorkspace>,
    trigger_integration: TriggerSender,
    conductor_handle: ConductorHandle,
    network: HolochainP2pDna,
    dht_query_cache: DhtDbQueryCache,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> WorkflowResult<WorkComplete> {
    let outcome_summary = app_validation_workflow_inner(
        dna_hash,
        workspace,
        conductor_handle,
        &network,
        dht_query_cache,
        validation_dependencies.clone(),
    )
    .await?;
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // if ops have been accepted or rejected, trigger integration
    if outcome_summary.validated > 0 {
        trigger_integration.trigger(&"app_validation_workflow");
    }

    Ok(
        // If not all ops have been validated
        // and fetching missing hashes has not timed out,
        // trigger app validation workflow again.
        if outcome_summary.validated < outcome_summary.ops_to_validate
            && !validation_dependencies
                .lock()
                .fetch_missing_hashes_timed_out()
        {
            // trigger app validation workflow again in 10 seconds
            WorkComplete::Incomplete(Some(Duration::from_secs(10)))
        } else {
            WorkComplete::Complete
        },
    )
}

async fn app_validation_workflow_inner(
    dna_hash: Arc<DnaHash>,
    workspace: Arc<AppValidationWorkspace>,
    conductor: ConductorHandle,
    network: &HolochainP2pDna,
    dht_query_cache: DhtDbQueryCache,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> WorkflowResult<OutcomeSummary> {
    let db = workspace.dht_db.clone().into();
    let sorted_dht_ops = validation_query::get_ops_to_app_validate(&db).await?;
    // filter out ops that have missing dependencies
    tracing::debug!("number of ops to validate {:?}", sorted_dht_ops.len());
    let sorted_dht_ops = validation_dependencies
        .lock()
        .filter_ops_missing_dependencies(sorted_dht_ops);
    let num_ops_to_validate = sorted_dht_ops.len();
    tracing::debug!(
        "number of ops to validate after filtering out ops missing hashes {num_ops_to_validate}"
    );
    tracing::trace!(
        "missing hashes: {:?}",
        validation_dependencies.lock().get_missing_hashes()
    );
    let sleuth_id = conductor.config.sleuth_id();

    let cascade = Arc::new(workspace.full_cascade(network.clone()));
    let validation_dependencies = validation_dependencies.clone();
    let accepted_ops = Arc::new(AtomicUsize::new(0));
    let awaiting_ops = Arc::new(AtomicUsize::new(0));
    let rejected_ops = Arc::new(AtomicUsize::new(0));
    let mut agent_activity = Vec::new();

    // Validate ops sequentially
    for sorted_dht_op in sorted_dht_ops.into_iter() {
        let (dht_op, dht_op_hash) = sorted_dht_op.into_inner();
        let op_type = dht_op.get_type();
        let action = dht_op.action();
        let dependency = dht_op.sys_validation_dependency();
        let dht_op_lite = dht_op.to_lite();

        // If this is agent activity, track it for the cache.
        let activity = matches!(op_type, DhtOpType::RegisterAgentActivity).then(|| {
            (
                action.author().clone(),
                action.action_seq(),
                dependency.is_none(),
            )
        });

        // Validate this op
        let validation_outcome = match dhtop_to_op(dht_op.clone(), cascade.clone()).await {
            Ok(op) => {
                validate_op_outer(
                    dna_hash.clone(),
                    &op,
                    &dht_op_hash,
                    &conductor,
                    &workspace,
                    network,
                    validation_dependencies.clone(),
                )
                .await
            }
            Err(e) => Err(e),
        };
        // Flatten nested app validation outcome to either ok or error
        let validation_outcome = match validation_outcome {
            Ok(outcome) => AppValidationResult::Ok(outcome),
            Err(OutcomeOrError::Outcome(outcome)) => AppValidationResult::Ok(outcome),
            Err(OutcomeOrError::Err(err)) => AppValidationResult::Err(err),
        };

        let sleuth_id = sleuth_id.clone();
        match validation_outcome {
            Ok(outcome) => {
                // Collect all agent activity.
                if let Some(activity) = activity {
                    // If the activity is accepted or rejected then it's ready to integrate.
                    if matches!(&outcome, Outcome::Accepted | Outcome::Rejected(_)) {
                        agent_activity.push(activity);
                    }
                }
                if let Outcome::AwaitingDeps(_) | Outcome::Rejected(_) = &outcome {
                    warn!(?outcome, ?dht_op_lite, "DhtOp has failed app validation");
                }

                let accepted_ops = accepted_ops.clone();
                let awaiting_ops = awaiting_ops.clone();
                let rejected_ops = rejected_ops.clone();

                let write_result = workspace
                    .dht_db
                    .write_async(move|txn| match outcome {
                        Outcome::Accepted => {
                            accepted_ops.fetch_add(1, Ordering::SeqCst);
                            aitia::trace!(&hc_sleuth::Event::AppValidated {
                                by: sleuth_id.clone(),
                                op: dht_op_hash.clone()
                            });

                            if dependency.is_none() {
                                aitia::trace!(&hc_sleuth::Event::Integrated {
                                    by: sleuth_id.clone(),
                                    op: dht_op_hash.clone()
                                });

                                put_integrated(txn, &dht_op_hash, ValidationStatus::Valid)
                            } else {
                                put_integration_limbo(txn, &dht_op_hash, ValidationStatus::Valid)
                            }
                        }
                        Outcome::AwaitingDeps(deps) => {
                            awaiting_ops.fetch_add(1, Ordering::SeqCst);
                            put_validation_limbo(
                                txn,
                                &dht_op_hash,
                                ValidationStage::AwaitingAppDeps(deps),
                            )
                        }
                        Outcome::Rejected(_) => {
                            rejected_ops.fetch_add(1, Ordering::SeqCst);
                            tracing::info!(
                            "Received invalid op. The op author will be blocked. Op: {dht_op_lite:?}"
                        );
                            if dependency.is_none() {
                                put_integrated(txn, &dht_op_hash, ValidationStatus::Rejected)
                            } else {
                                put_integration_limbo(txn, &dht_op_hash, ValidationStatus::Rejected)
                            }
                        }
                    })
                    .await;
                if let Err(err) = write_result {
                    tracing::error!(?dht_op, ?err, "Error updating dht op in database.");
                }
            }
            Err(err) => {
                tracing::error!(
                    ?dht_op,
                    ?err,
                    "App validation error when validating dht op."
                );
            }
        }
    }

    // Once the database transaction is committed, add agent activity to the cache
    // that is ready for integration.
    for (author, seq, has_no_dependency) in agent_activity {
        // Any activity with no dependency is integrated in this workflow.
        // TODO: This will no longer be true when [#1212](https://github.com/holochain/holochain/pull/1212) lands.
        if has_no_dependency {
            dht_query_cache
                .set_activity_to_integrated(&author, seq)
                .await?;
        } else {
            dht_query_cache
                .set_activity_ready_to_integrate(&author, seq)
                .await?;
        }
    }

    let accepted_ops = accepted_ops.load(Ordering::SeqCst);
    let awaiting_ops = awaiting_ops.load(Ordering::SeqCst);
    let rejected_ops = rejected_ops.load(Ordering::SeqCst);
    let ops_validated = accepted_ops + rejected_ops;
    tracing::info!("{ops_validated} out of {num_ops_to_validate} validated: {accepted_ops} accepted, {awaiting_ops} awaiting deps, {rejected_ops} rejected.");

    let outcome_summary = OutcomeSummary {
        ops_to_validate: num_ops_to_validate,
        validated: ops_validated,
        accepted: accepted_ops,
        missing: awaiting_ops,
        rejected: rejected_ops,
    };
    Ok(outcome_summary)
}

pub async fn record_to_op(
    record: Record,
    op_type: DhtOpType,
    cascade: Arc<impl Cascade>,
) -> AppValidationOutcome<(Op, DhtOpHash, Option<Entry>)> {
    use DhtOpType::*;

    // Hide private data where appropriate
    let (record, mut hidden_entry) = if matches!(op_type, DhtOpType::StoreEntry) {
        // We don't want to hide private data for a StoreEntry, because when doing
        // inline validation as an author, we want to validate and integrate our own entry!
        // Publishing and gossip rules state that a private StoreEntry will never be transmitted
        // to another node.
        (record, None)
    } else {
        // All other records have private entry data hidden, including from ourselves if we are
        // authoring private data.
        record.privatized()
    };

    let (shh, entry) = record.into_inner();
    let mut entry = entry.into_option();
    let action = shh.into();
    // Register agent activity doesn't store the entry so we need to
    // save it so we can reconstruct the record later.
    if matches!(op_type, RegisterAgentActivity) {
        hidden_entry = entry.take().or(hidden_entry);
    }
    let dht_op = DhtOp::from_type(op_type, action, entry)?;
    let dht_op_hash = dht_op.clone().to_hash();
    Ok((
        dhtop_to_op(dht_op, cascade).await?,
        dht_op_hash,
        hidden_entry,
    ))
}

async fn dhtop_to_op(dht_op: DhtOp, cascade: Arc<impl Cascade>) -> AppValidationOutcome<Op> {
    let op = match dht_op {
        DhtOp::StoreRecord(signature, action, entry) => Op::StoreRecord(StoreRecord {
            record: Record::new(
                SignedActionHashed::with_presigned(
                    ActionHashed::from_content_sync(action),
                    signature,
                ),
                entry.into_option(),
            ),
        }),
        DhtOp::StoreEntry(signature, action, entry) => Op::StoreEntry(StoreEntry {
            action: SignedHashed::new_unchecked(action.into(), signature),
            entry,
        }),
        DhtOp::RegisterAgentActivity(signature, action) => {
            Op::RegisterAgentActivity(RegisterAgentActivity {
                action: SignedActionHashed::with_presigned(
                    ActionHashed::from_content_sync(action),
                    signature,
                ),
                cached_entry: None,
            })
        }
        DhtOp::RegisterUpdatedContent(signature, update, entry)
        | DhtOp::RegisterUpdatedRecord(signature, update, entry) => {
            let new_entry = match update.entry_type.visibility() {
                EntryVisibility::Public => match entry.into_option() {
                    Some(entry) => Some(entry),
                    None => Some(
                        cascade
                            .retrieve_entry(update.entry_hash.clone(), Default::default())
                            .await?
                            .map(|(e, _)| e.into_content())
                            .ok_or_else(|| Outcome::awaiting(&update.entry_hash))?,
                    ),
                },
                _ => None,
            };
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed::new_unchecked(update, signature),
                new_entry,
            })
        }
        DhtOp::RegisterDeletedBy(signature, delete)
        | DhtOp::RegisterDeletedEntryAction(signature, delete) => {
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed::new_unchecked(delete, signature),
            })
        }
        DhtOp::RegisterAddLink(signature, create_link) => {
            Op::RegisterCreateLink(RegisterCreateLink {
                create_link: SignedHashed::new_unchecked(create_link, signature),
            })
        }
        DhtOp::RegisterRemoveLink(signature, delete_link) => {
            let create_link = cascade
                .retrieve_action(delete_link.link_add_address.clone(), Default::default())
                .await?
                .and_then(|(sh, _)| CreateLink::try_from(sh.hashed.content).ok())
                .ok_or_else(|| Outcome::awaiting(&delete_link.link_add_address))?;
            Op::RegisterDeleteLink(RegisterDeleteLink {
                delete_link: SignedHashed::new_unchecked(delete_link, signature),
                create_link,
            })
        }
    };
    Ok(op)
}

async fn validate_op_outer(
    dna_hash: Arc<DnaHash>,
    op: &Op,
    dht_op_hash: &DhtOpHash,
    conductor_handle: &ConductorHandle,
    workspace: &AppValidationWorkspace,
    network: &HolochainP2pDna,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> AppValidationOutcome<Outcome> {
    // Get the workspace for the validation calls
    let host_fn_workspace = workspace.validation_workspace().await?;

    // Get the ribosome
    let ribosome = conductor_handle
        .get_ribosome(dna_hash.as_ref())
        .map_err(|_| AppValidationError::DnaMissing((*dna_hash).clone()))?;

    validate_op(
        op,
        dht_op_hash,
        host_fn_workspace,
        network,
        &ribosome,
        conductor_handle,
        validation_dependencies,
    )
    .await
}

pub async fn validate_op(
    op: &Op,
    dht_op_hash: &DhtOpHash,
    workspace: HostFnWorkspaceRead,
    network: &HolochainP2pDna,
    ribosome: &impl RibosomeT,
    conductor_handle: &ConductorHandle,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> AppValidationOutcome<Outcome> {
    check_entry_def(op, &network.dna_hash(), conductor_handle)
        .await
        .map_err(AppValidationError::SysValidationError)?;

    let network = Arc::new(network.clone());

    let zomes_to_invoke = get_zomes_to_invoke(op, &workspace, network.clone(), ribosome).await;
    if let Err(OutcomeOrError::Err(err)) = &zomes_to_invoke {
        tracing::error!(?op, ?err, "Error getting zomes to invoke to validate op.");
    };
    let zomes_to_invoke = zomes_to_invoke?;
    let invocation = ValidateInvocation::new(zomes_to_invoke, op)
        .map_err(|e| AppValidationError::RibosomeError(e.into()))?;

    let outcome = run_validation_callback(
        invocation,
        dht_op_hash,
        ribosome,
        workspace,
        network,
        validation_dependencies,
    )
    .await?;

    Ok(outcome)
}

/// Check the AppEntryDef is valid for the zome.
/// Check the EntryDefId and ZomeIndex are in range.
async fn check_entry_def(
    op: &Op,
    dna_hash: &DnaHash,
    conductor: &Conductor,
) -> SysValidationResult<()> {
    if let Some((_, EntryType::App(app_entry_def))) = op.entry_data() {
        check_app_entry_def(app_entry_def, dna_hash, conductor).await
    } else {
        Ok(())
    }
}

/// Check the AppEntryDef is valid for the zome.
/// Check the EntryDefId and ZomeIndex are in range.
async fn check_app_entry_def(
    app_entry_def: &AppEntryDef,
    dna_hash: &DnaHash,
    conductor: &Conductor,
) -> SysValidationResult<()> {
    // We want to be careful about holding locks open to the conductor api
    // so calls are made in blocks
    let ribosome = conductor
        .get_ribosome(dna_hash)
        .map_err(|_| SysValidationError::DnaMissing(dna_hash.clone()))?;

    // Check if the zome is found
    let zome = ribosome
        .get_integrity_zome(&app_entry_def.zome_index())
        .ok_or_else(|| ValidationOutcome::ZomeIndex(app_entry_def.clone()))?
        .into_inner()
        .1;

    let entry_def = get_entry_def(app_entry_def.entry_index(), zome, dna_hash, conductor).await?;

    // Check the visibility and return
    match entry_def {
        Some(entry_def) => {
            if entry_def.visibility == *app_entry_def.visibility() {
                Ok(())
            } else {
                Err(ValidationOutcome::EntryVisibility(app_entry_def.clone()).into())
            }
        }
        None => Err(ValidationOutcome::EntryDefId(app_entry_def.clone()).into()),
    }
}

// Zomes to invoke for app validation are determined based on app entries'
// zome index. Whenever an app entry is contained in the op, the zome index can
// directly be known. For other cases like deletes, the deleted action is
// retrieved with the expectation that it is the original create or an update,
// which again include the app entry type that specifies the zome index of the
// integrity zome.
//
// Special cases are non app entries like cap grants and claims and agent pub
// keys. None of them have an entry definition or a zome index of the integrity
// zome. Thus all integrity zomes are returned for validation invocation.
async fn get_zomes_to_invoke(
    op: &Op,
    workspace: &HostFnWorkspaceRead,
    network: GenericNetwork,
    ribosome: &impl RibosomeT,
) -> AppValidationOutcome<ZomesToInvoke> {
    match op {
        Op::RegisterAgentActivity(RegisterAgentActivity { .. }) => Ok(ZomesToInvoke::AllIntegrity),
        Op::StoreRecord(StoreRecord { record }) => {
            // For deletes there is no entry type to check, so we get the previous action.
            // In theory this can be yet another delete, in which case all
            // integrity zomes are returned for invocation.
            // Instead the delete could be followed up the chain to find the original
            // create, but since deleting a delete does not have much practical use,
            // it is neglected here.
            let action = match record.action() {
                Action::Delete(Delete {
                    deletes_address, ..
                })
                | Action::DeleteLink(DeleteLink {
                    link_add_address: deletes_address,
                    ..
                }) => {
                    let deleted_action =
                        retrieve_deleted_action(workspace, network, deletes_address).await?;
                    deleted_action.action().clone()
                }
                _ => record.action().clone(),
            };

            match action {
                Action::CreateLink(CreateLink { zome_index, .. })
                | Action::Create(Create {
                    entry_type: EntryType::App(AppEntryDef { zome_index, .. }),
                    ..
                })
                | Action::Update(Update {
                    entry_type: EntryType::App(AppEntryDef { zome_index, .. }),
                    ..
                }) => get_integrity_zome_from_ribosome(&zome_index, ribosome),
                _ => Ok(ZomesToInvoke::AllIntegrity),
            }
        }
        Op::StoreEntry(StoreEntry { action, .. }) => match &action.hashed.content {
            EntryCreationAction::Create(Create {
                entry_type: EntryType::App(app_entry_def),
                ..
            })
            | EntryCreationAction::Update(Update {
                entry_type: EntryType::App(app_entry_def),
                ..
            }) => get_integrity_zome_from_ribosome(&app_entry_def.zome_index, ribosome),
            _ => Ok(ZomesToInvoke::AllIntegrity),
        },
        Op::RegisterUpdate(RegisterUpdate { update, .. }) => match &update.hashed.entry_type {
            EntryType::App(app_entry_def) => {
                get_integrity_zome_from_ribosome(&app_entry_def.zome_index, ribosome)
            }
            _ => Ok(ZomesToInvoke::AllIntegrity),
        },
        Op::RegisterDelete(RegisterDelete { delete }) => {
            let deletes_address = &delete.hashed.deletes_address;
            let deleted_action =
                retrieve_deleted_action(workspace, network, deletes_address).await?;
            match deleted_action.hashed.content {
                Action::Create(Create {
                    entry_type: EntryType::App(app_entry_def),
                    ..
                })
                | Action::Update(Update {
                    entry_type: EntryType::App(app_entry_def),
                    ..
                }) => get_integrity_zome_from_ribosome(&app_entry_def.zome_index, ribosome),
                _ => Ok(ZomesToInvoke::AllIntegrity),
            }
        }
        Op::RegisterCreateLink(RegisterCreateLink {
            create_link:
                SignedHashed {
                    hashed:
                        HoloHashed {
                            content: action, ..
                        },
                    ..
                },
            ..
        })
        | Op::RegisterDeleteLink(RegisterDeleteLink {
            create_link: action,
            ..
        }) => get_integrity_zome_from_ribosome(&action.zome_index, ribosome),
    }
}

async fn retrieve_deleted_action(
    workspace: &HostFnWorkspaceRead,
    network: GenericNetwork,
    deletes_address: &ActionHash,
) -> AppValidationOutcome<SignedActionHashed> {
    let cascade = CascadeImpl::from_workspace_and_network(workspace, network.clone());
    let (deleted_action, _) = cascade
        .retrieve_action(deletes_address.clone(), NetworkGetOptions::default())
        .await?
        .ok_or_else(|| Outcome::awaiting(deletes_address))?;
    Ok(deleted_action)
}

fn get_integrity_zome_from_ribosome(
    zome_index: &ZomeIndex,
    ribosome: &impl RibosomeT,
) -> AppValidationOutcome<ZomesToInvoke> {
    let zome = ribosome.get_integrity_zome(zome_index).ok_or_else(|| {
        Outcome::rejected(format!("No integrity zome found with index {zome_index:?}"))
    })?;
    Ok(ZomesToInvoke::OneIntegrity(zome))
}

async fn run_validation_callback(
    invocation: ValidateInvocation,
    dht_op_hash: &DhtOpHash,
    ribosome: &impl RibosomeT,
    workspace: HostFnWorkspaceRead,
    network: GenericNetwork,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> AppValidationResult<Outcome> {
    let validate_result = ribosome.run_validate(
        ValidateHostAccess::new(workspace.clone(), network.clone()),
        invocation.clone(),
    )?;
    match validate_result {
        ValidateResult::Valid => Ok(Outcome::Accepted),
        ValidateResult::Invalid(reason) => Ok(Outcome::Rejected(reason)),
        ValidateResult::UnresolvedDependencies(UnresolvedDependencies::Hashes(hashes)) => {
            tracing::debug!(
                ?hashes,
                "Op validation returned unresolved dependencies -  AgentActivity"
            );
            // fetch all missing hashes in the background without awaiting them
            let cascade_workspace = workspace.clone();
            let cascade =
                CascadeImpl::from_workspace_and_network(&cascade_workspace, network.clone());

            // keep track of which dependencies are being fetched for which dht op and to
            // prevent multiple fetches of the same hash
            let new_hashes_to_fetch = validation_dependencies
                .lock()
                .filter_missing_hashes_to_fetch_for_op(hashes.clone(), dht_op_hash.clone());

            // build a collection of futures to fetch the missing hashes
            let fetches = new_hashes_to_fetch.into_iter().map(move |hash| {
                let cascade = cascade.clone();
                let validation_dependencies = validation_dependencies.clone();
                async move {
                    let result = cascade
                        .fetch_record(hash.clone(), NetworkGetOptions::must_get_options())
                        .await;
                    if let Err(err) = result {
                        tracing::warn!("error fetching dependent hash {hash:?}: {err}");
                    }
                    // Dependency has been fetched and added to the cache
                    // or an error occurred along the way.
                    // In case of an error the hash is still removed from
                    // the collection so that it will be tried again to be
                    // fetched in the subsequent workflow run.
                    validation_dependencies.lock().remove_missing_hash(&hash);
                    // Secondly remove the just fetched hash from the set
                    // of missing hashes for the op
                }
            });
            // await all fetches in a separate task in the background
            tokio::spawn(async { futures::future::join_all(fetches).await });
            Ok(Outcome::AwaitingDeps(hashes))
        }
        ValidateResult::UnresolvedDependencies(UnresolvedDependencies::AgentActivity(
            author,
            filter,
        )) => {
            tracing::debug!(
                ?author,
                ?filter,
                "Op validation returned unresolved dependencies -  AgentActivity"
            );
            // fetch missing agent activities in the background without awaiting them
            let cascade_workspace = workspace.clone();
            let author = author.clone();
            let cascade =
                CascadeImpl::from_workspace_and_network(&cascade_workspace, network.clone());

            // keep track of which dependencies are being fetched to
            // prevent multiple fetches of the same hash
            let validation_dependencies = validation_dependencies.clone();
            let is_new_dependency = validation_dependencies
                .lock()
                .insert_missing_hash_for_op(author.clone().into(), dht_op_hash.clone());
            // fetch dependency if it is not being fetched yet
            if is_new_dependency {
                tokio::spawn({
                    let author = author.clone();
                    async move {
                        let result = cascade
                            .must_get_agent_activity(author.clone(), filter)
                            .await;
                        if let Err(err) = result {
                            tracing::warn!(
                                "error fetching dependent chain of agent {author:?}: {err}"
                            );
                        }
                        // dependency has been fetched and added to the cache
                        // or an error occurred along the way; in case of an
                        // error the hash is still removed from the
                        // collection so that it will be tried again to be
                        // fetched in the subsequent workflow run
                        validation_dependencies
                            .lock()
                            .remove_missing_hash(&author.into());
                    }
                });
            }
            Ok(Outcome::AwaitingDeps(vec![author.into()]))
        }
    }
}

// accepted, missing and rejected are only used in tests
#[allow(dead_code)]
#[derive(Debug)]
struct OutcomeSummary {
    ops_to_validate: usize,
    validated: usize,
    accepted: usize,
    missing: usize,
    rejected: usize,
}

impl OutcomeSummary {
    fn new() -> Self {
        OutcomeSummary {
            ops_to_validate: 0,
            validated: 0,
            accepted: 0,
            missing: 0,
            rejected: 0,
        }
    }
}

impl Default for OutcomeSummary {
    fn default() -> Self {
        OutcomeSummary::new()
    }
}

pub struct AppValidationWorkspace {
    authored_db: DbRead<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    dht_db_cache: DhtDbQueryCache,
    cache: DbWrite<DbKindCache>,
    keystore: MetaLairClient,
    dna_def: Arc<DnaDef>,
}

impl AppValidationWorkspace {
    pub fn new(
        authored_db: DbRead<DbKindAuthored>,
        dht_db: DbWrite<DbKindDht>,
        dht_db_cache: DhtDbQueryCache,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        dna_def: Arc<DnaDef>,
    ) -> Self {
        Self {
            authored_db,
            dht_db,
            dht_db_cache,
            cache,
            keystore,
            dna_def,
        }
    }

    pub async fn validation_workspace(&self) -> AppValidationResult<HostFnWorkspaceRead> {
        Ok(HostFnWorkspace::new(
            self.authored_db.clone(),
            self.dht_db.clone().into(),
            self.dht_db_cache.clone(),
            self.cache.clone(),
            self.keystore.clone(),
            None,
            self.dna_def.clone(),
        )
        .await?)
    }

    pub fn full_cascade<Network: HolochainP2pDnaT>(&self, network: Network) -> CascadeImpl {
        CascadeImpl::empty()
            .with_authored(self.authored_db.clone())
            .with_dht(self.dht_db.clone().into())
            .with_network(Arc::new(network), self.cache.clone())
    }
}

pub fn put_validation_limbo(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    status: ValidationStage,
) -> WorkflowResult<()> {
    set_validation_stage(txn, hash, status)?;
    Ok(())
}

pub fn put_integration_limbo(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    status: ValidationStatus,
) -> WorkflowResult<()> {
    set_validation_status(txn, hash, status)?;
    set_validation_stage(txn, hash, ValidationStage::AwaitingIntegration)?;
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
    set_validation_stage(txn, hash, ValidationStage::Pending)?;
    set_when_integrated(txn, hash, Timestamp::now())?;

    // If the op is rejected then force a receipt to be processed because the
    // receipt is a warrant, so of course the author won't want it to be
    // produced.
    if matches!(status, ValidationStatus::Rejected) {
        set_require_receipt(txn, hash, true)?;
    }
    Ok(())
}
