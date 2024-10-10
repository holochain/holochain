//! Holochain workflow to validate all incoming DHT operations with an
//! app-defined validation function.
//!
//! Triggered by system validation, this workflow iterates over a list of
//! [`DhtOp`]s that have passed system validation, validates each op, updates its validation status
//! in the database accordingly, and triggers op integration if necessary.
//!
//! ### Sequential validation
//!

// Even though ops are validated in sequence, they could all be validated in parallel too with the same result.
// All actions are written to the database straight away in the incoming dht ops workflow and do not require validation to be available for validating other ops. See https://github.com/holochain/holochain/issues/3724

//! Ops are validated in sequence based on their op type and the timestamp they
//! were authored (see [`OpOrder`] and [`OpNumericalOrder`]). Validating one op
//! after the other with this ordering was chosen so that ops that depend on earlier
//! ops will be validated after the earlier ops, and therefore have a higher chance
//! of being validated successfully. An example is an incoming delete
//! op that depends on a create op. Validated in order of their authoring, the
//! create op is validated first, followed at some stage by the delete op. If
//! the validation function references the original action when validating
//! delete ops, the create op will have been validated and is available in the
//! database. Otherwise the delete op could not be validated and its dependency,
//! the create op, would be awaited first.
//!
//! ### Op validation
//!
//! For each op the [corresponding app validation function](https://docs.rs/hdi/latest/hdi/#data-validation)
//! is executed. Entry and link CRUD actions, which the ops are derived from, have been
//! written with a particular integrity zome's entry and link types. Thus for
//! op validation, the validation function of the same integrity zome must be
//! used. Ops that do not relate to a specific entry or link like [`ChainOp::RegisterAgentActivity`]
//! or non-app entries like [`EntryType::CapGrant`] are validated with all
//! validation functions of the DNA's integrity zomes.
//!
//! Having established the relevant integrity zomes for validating an op, each
//! zome's validation callback is invoked.
//!
//! ### Outcome
//!
//! An op can be valid or invalid, which is considered "validated", or it could not be
//! validated because it is missing required dependencies such as actions,
//! entries, links or agent activity that the validation function is referencing. If
//! all ops were validated, the workflow completes with no further action. If
//! however some ops could not be validated, the workflow will trigger itself
//! again after a delay, while missing dependencies are being fetched in the
//! background.
//!
//! #### Errors
//!
//! If the validate invocation of an integrity zome returns an error while
//! validating an op, the op is considered not validated but also not missing
//! dependencies. In effect the workflow will not re-trigger itself.
//!
//! Such errors do not depend on op validity or presence of ops, but indicate
//! a more fundamental problem with either the conductor state, like a missing
//! zome or ribosome or DNA, or network access. For none of these errors is the
//! conductor able to recover itself.
//!
//! Ops that have not been validated due to validation errors will be retried
//! the next time app validation runs, when other ops from gossip or publish come in and
//! need to be validated.
//!
//! ### Missing dependencies
//!
//! Finding the zomes to invoke for validation oftentimes involves fetching a
//! referenced original action, like in the case of updates and deletes. Further
//! the validation function may require actions, entries or agent activity
//! (segments of an agent's source chain) that currently are not stored in the
//! local databases. These are dependencies of the op. If they are missing
//! locally, a network get request will be sent in the background. The op
//! validation outcome will be [`Outcome::AwaitingDeps`]. Validation of
//! remaining ops will carry on, as the network request's response is not
//! awaited within the op validation loop. Instead the whole workflow triggers
//! itself again after a delay.
//!
//! ### Workflow re-triggering
//!
//! Missing dependencies of ops re-trigger the validation workflow. After a delay of
//! a maximum of 3 seconds, which gives the background task that gets the missing
//! dependencies from the network some time to complete, the app validation workflow
//! runs again. All ops whose missing dependencies could be fetched during this interval
//! will successfully validate now.
//!
//! ### Integration workflow
//!

// This seems to mainly affect ops with system dependencies, as ops without such
// dependencies are set to integrated as part of this workflow.

//! If any ops have been validated (outcome valid or invalid), [`integrate_dht_ops_workflow`](crate::core::workflow::integrate_dht_ops_workflow)
//! is triggered, which completes integration of ops after successful validation.

use super::error::WorkflowResult;
use super::sys_validation_workflow::validation_query;

use crate::conductor::api::DpkiApi;
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
use std::collections::HashSet;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tracing::*;

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

#[cfg_attr(
    feature = "instrument",
    instrument(skip(
        workspace,
        trigger_integration,
        trigger_publish,
        conductor_handle,
        network,
        dht_query_cache,
    ))
)]
#[allow(clippy::too_many_arguments)]
pub async fn app_validation_workflow(
    dna_hash: Arc<DnaHash>,
    workspace: Arc<AppValidationWorkspace>,
    trigger_integration: TriggerSender,
    trigger_publish: TriggerSender,
    conductor_handle: ConductorHandle,
    network: HolochainP2pDna,
    dht_query_cache: DhtDbQueryCache,
) -> WorkflowResult<WorkComplete> {
    let outcome_summary = app_validation_workflow_inner(
        dna_hash,
        workspace,
        conductor_handle,
        &network,
        dht_query_cache,
    )
    .await?;
    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // If ops have been accepted or rejected, trigger integration.
    if outcome_summary.validated > 0 {
        trigger_integration.trigger(&"app_validation_workflow");
    }

    // If ops have been warranted, trigger publishing.
    if outcome_summary.warranted > 0 {
        trigger_publish.trigger(&"app_validation_workflow");
    }

    Ok(
        // If not all ops have been validated, trigger app validation workflow again.
        if outcome_summary.validated < outcome_summary.ops_to_validate {
            // Trigger app validation workflow again in 100-3000 milliseconds.
            let interval = 2900u64.saturating_sub(outcome_summary.missing as u64 * 100) + 100;
            WorkComplete::Incomplete(Some(Duration::from_millis(interval)))
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
) -> WorkflowResult<OutcomeSummary> {
    let db = workspace.dht_db.clone().into();
    let sorted_dht_ops = validation_query::get_ops_to_app_validate(&db).await?;
    let num_ops_to_validate = sorted_dht_ops.len();

    let cascade = Arc::new(workspace.full_cascade(network.clone()));
    let accepted_ops = Arc::new(AtomicUsize::new(0));
    let awaiting_ops = Arc::new(AtomicUsize::new(0));
    let rejected_ops = Arc::new(AtomicUsize::new(0));
    let warranted_ops = Arc::new(AtomicUsize::new(0));
    let failed_ops = Arc::new(Mutex::new(HashSet::new()));
    let mut agent_activity = vec![];
    let mut warrant_op_hashes = vec![];

    // Validate ops sequentially
    for sorted_dht_op in sorted_dht_ops.into_iter() {
        let (dht_op, dht_op_hash) = sorted_dht_op.into_inner();
        let deps = dht_op.sys_validation_dependencies();

        let chain_op = match dht_op {
            DhtOp::ChainOp(chain_op) => chain_op,
            _ => unreachable!("warrant ops are never sent to app validation"),
        };

        let op_type = chain_op.get_type();
        let action = chain_op.action();
        let dht_op_lite = chain_op.to_lite();

        // If this is agent activity, track it for the cache.
        let activity = matches!(op_type, ChainOpType::RegisterAgentActivity).then(|| {
            (
                action.author().clone(),
                action.action_seq(),
                deps.is_empty(),
            )
        });

        // Validate this op
        let validation_outcome = match chain_op_to_op(*chain_op.clone(), cascade.clone()).await {
            Ok(op) => {
                validate_op_outer(dna_hash.clone(), &op, &conductor, &workspace, network).await
            }
            Err(e) => Err(e),
        };
        // Flatten nested app validation outcome to either ok or error
        let validation_outcome = match validation_outcome {
            Ok(outcome) => AppValidationResult::Ok(outcome),
            Err(OutcomeOrError::Outcome(outcome)) => AppValidationResult::Ok(outcome),
            Err(OutcomeOrError::Err(err)) => AppValidationResult::Err(err),
        };

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

                if let Outcome::Rejected(_) = &outcome {
                    let warrant_op =
                        crate::core::workflow::sys_validation_workflow::make_warrant_op(
                            &conductor,
                            &dna_hash,
                            &chain_op,
                            ValidationType::App,
                        )
                        .await?;

                    warrant_op_hashes.push((warrant_op.to_hash(), warrant_op.dht_basis().clone()));

                    workspace
                        .authored_db
                        .write_async(move |txn| {
                            warn!("Inserting warrant op");
                            insert_op(txn, &warrant_op)
                        })
                        .await?;

                    warranted_ops.fetch_add(1, Ordering::SeqCst);
                }

                let write_result = workspace
                    .dht_db
                    .write_async(move|txn| match outcome {
                        Outcome::Accepted => {
                            accepted_ops.fetch_add(1, Ordering::SeqCst);


                            if deps.is_empty() {

                                put_integrated(txn, &dht_op_hash, ValidationStatus::Valid)
                            } else {
                                put_integration_limbo(txn, &dht_op_hash, ValidationStatus::Valid)
                            }
                        }
                        Outcome::AwaitingDeps(_) => {
                            awaiting_ops.fetch_add(1, Ordering::SeqCst);
                            put_validation_limbo(
                                txn,
                                &dht_op_hash,
                                ValidationStage::AwaitingAppDeps,
                            )
                        }
                        Outcome::Rejected(_) => {
                            rejected_ops.fetch_add(1, Ordering::SeqCst);

                            tracing::info!("Received invalid op. The op author will be blocked. Op: {dht_op_lite:?}");

                            if deps.is_empty() {
                                put_integrated(txn, &dht_op_hash, ValidationStatus::Rejected)
                            } else {
                                put_integration_limbo(txn, &dht_op_hash, ValidationStatus::Rejected)
                            }
                        }
                    })
                    .await;
                if let Err(err) = write_result {
                    tracing::error!(?chain_op, ?err, "Error updating dht op in database.");
                }
            }
            Err(err) => {
                tracing::error!(
                    ?chain_op,
                    ?err,
                    "App validation error when validating dht op."
                );
                failed_ops.lock().insert(dht_op_hash);
            }
        }
    }

    // "self-publish" warrants, i.e. insert them into the DHT db as if they were published to us by another node
    holochain_state::integrate::authored_ops_to_dht_db(
        network,
        warrant_op_hashes,
        workspace.authored_db.clone().into(),
        workspace.dht_db.clone(),
        &workspace.dht_db_cache,
    )
    .await?;

    // Once the database transaction is committed, add agent activity to the cache
    // that is ready for integration.
    for (author, seq, has_no_dependency) in agent_activity {
        // Any activity with no dependency is integrated in this workflow.
        // TODO: This will no longer be true when [#1212](https://github.com/holochain/holochain/pull/1212) lands.
        if has_no_dependency {
            dht_query_cache
                .set_activity_to_integrated(&author, Some(seq))
                .await?;
        } else {
            dht_query_cache
                .set_activity_ready_to_integrate(&author, Some(seq))
                .await?;
        }
    }

    let accepted_ops = accepted_ops.load(Ordering::SeqCst);
    let awaiting_ops = awaiting_ops.load(Ordering::SeqCst);
    let rejected_ops = rejected_ops.load(Ordering::SeqCst);
    let warranted_ops = warranted_ops.load(Ordering::SeqCst);
    let ops_validated = accepted_ops + rejected_ops;
    let failed_ops = Arc::try_unwrap(failed_ops)
        .expect("must be only reference")
        .into_inner();
    tracing::info!("{ops_validated} out of {num_ops_to_validate} validated: {accepted_ops} accepted, {awaiting_ops} awaiting deps, {rejected_ops} rejected, failed ops {failed_ops:?}.");

    let outcome_summary = OutcomeSummary {
        ops_to_validate: num_ops_to_validate,
        validated: ops_validated,
        accepted: accepted_ops,
        missing: awaiting_ops,
        rejected: rejected_ops,
        warranted: warranted_ops,
        failed: failed_ops,
    };
    Ok(outcome_summary)
}

// This fn is only used in the zome call workflow's inline validation.
pub async fn record_to_op(
    record: Record,
    op_type: ChainOpType,
    cascade: Arc<impl Cascade>,
) -> AppValidationOutcome<(Op, DhtOpHash, Option<Entry>)> {
    // Hide private data where appropriate
    let (record, mut hidden_entry) = if matches!(op_type, ChainOpType::StoreEntry) {
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

    let (sah, entry) = record.into_inner();
    let mut entry = entry.into_option();
    let action = sah.into();
    // Register agent activity doesn't store the entry so we need to
    // save it so we can reconstruct the record later.
    if matches!(op_type, ChainOpType::RegisterAgentActivity) {
        hidden_entry = entry.take().or(hidden_entry);
    }
    let chain_op = ChainOp::from_type(op_type, action, entry)?;
    let chain_op_hash = chain_op.clone().to_hash();
    Ok((
        chain_op_to_op(chain_op, cascade).await?,
        chain_op_hash,
        hidden_entry,
    ))
}

async fn chain_op_to_op(chain_op: ChainOp, cascade: Arc<impl Cascade>) -> AppValidationOutcome<Op> {
    let op = match chain_op {
        ChainOp::StoreRecord(signature, action, entry) => Op::StoreRecord(StoreRecord {
            record: Record::new(
                SignedActionHashed::with_presigned(
                    ActionHashed::from_content_sync(action),
                    signature,
                ),
                entry.into_option(),
            ),
        }),
        ChainOp::StoreEntry(signature, action, entry) => Op::StoreEntry(StoreEntry {
            action: SignedHashed::new_unchecked(action.into(), signature),
            entry,
        }),
        ChainOp::RegisterAgentActivity(signature, action) => {
            Op::RegisterAgentActivity(RegisterAgentActivity {
                action: SignedActionHashed::with_presigned(
                    ActionHashed::from_content_sync(action),
                    signature,
                ),
                cached_entry: None,
            })
        }
        ChainOp::RegisterUpdatedContent(signature, update, entry)
        | ChainOp::RegisterUpdatedRecord(signature, update, entry) => {
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
        ChainOp::RegisterDeletedBy(signature, delete)
        | ChainOp::RegisterDeletedEntryAction(signature, delete) => {
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed::new_unchecked(delete, signature),
            })
        }
        ChainOp::RegisterAddLink(signature, create_link) => {
            Op::RegisterCreateLink(RegisterCreateLink {
                create_link: SignedHashed::new_unchecked(create_link, signature),
            })
        }
        ChainOp::RegisterRemoveLink(signature, delete_link) => {
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
    conductor_handle: &ConductorHandle,
    workspace: &AppValidationWorkspace,
    network: &HolochainP2pDna,
) -> AppValidationOutcome<Outcome> {
    // Get the workspace for the validation calls
    let host_fn_workspace = workspace.validation_workspace().await?;

    // Get the ribosome
    let ribosome = conductor_handle
        .get_ribosome(dna_hash.as_ref())
        .map_err(|_| AppValidationError::DnaMissing((*dna_hash).clone()))?;

    let dpki = conductor_handle.running_services().dpki;

    validate_op(
        op,
        host_fn_workspace,
        network,
        &ribosome,
        conductor_handle,
        dpki,
        false, // is_inline
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn validate_op(
    op: &Op,
    workspace: HostFnWorkspaceRead,
    network: &HolochainP2pDna,
    ribosome: &impl RibosomeT,
    conductor_handle: &ConductorHandle,
    dpki: DpkiApi,
    is_inline: bool,
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

    let outcome =
        run_validation_callback(invocation, ribosome, workspace, network, dpki, is_inline).await?;

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

#[allow(clippy::too_many_arguments)]
async fn run_validation_callback(
    invocation: ValidateInvocation,
    ribosome: &impl RibosomeT,
    workspace: HostFnWorkspaceRead,
    network: GenericNetwork,
    dpki: DpkiApi,
    is_inline: bool,
) -> AppValidationResult<Outcome> {
    let validate_result = ribosome
        .run_validate(
            ValidateHostAccess::new(workspace.clone(), network.clone(), dpki, is_inline),
            invocation.clone(),
        )
        .await?;
    match validate_result {
        ValidateResult::Valid => Ok(Outcome::Accepted),
        ValidateResult::Invalid(reason) => Ok(Outcome::Rejected(reason)),
        ValidateResult::UnresolvedDependencies(UnresolvedDependencies::Hashes(hashes)) => {
            tracing::debug!(
                ?hashes,
                "Op validation returned unresolved dependencies -  Hashes"
            );
            // fetch all missing hashes in the background without awaiting them
            let cascade_workspace = workspace.clone();
            let cascade =
                CascadeImpl::from_workspace_and_network(&cascade_workspace, network.clone());

            // build a collection of futures to fetch the missing hashes
            let fetches = hashes.clone().into_iter().map(move |hash| {
                let cascade = cascade.clone();
                async move {
                    let result = cascade
                        .fetch_record(hash.clone(), NetworkGetOptions::must_get_options())
                        .await;
                    if let Err(err) = result {
                        tracing::warn!("error fetching dependent hash {hash:?}: {err}");
                    }
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

            // fetch dependency
            tokio::spawn({
                let author = author.clone();
                async move {
                    let result = cascade
                        .must_get_agent_activity(author.clone(), filter)
                        .await;
                    if let Err(err) = result {
                        tracing::warn!("error fetching dependent chain of agent {author:?}: {err}");
                    }
                }
            });
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
    warranted: usize,
    failed: HashSet<DhtOpHash>,
}

impl OutcomeSummary {
    fn new() -> Self {
        OutcomeSummary {
            ops_to_validate: 0,
            validated: 0,
            accepted: 0,
            missing: 0,
            rejected: 0,
            warranted: 0,
            failed: HashSet::new(),
        }
    }
}

impl Default for OutcomeSummary {
    fn default() -> Self {
        OutcomeSummary::new()
    }
}

pub struct AppValidationWorkspace {
    // Writeable because of warrants
    authored_db: DbWrite<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    dht_db_cache: DhtDbQueryCache,
    cache: DbWrite<DbKindCache>,
    keystore: MetaLairClient,
    _dna_def: Arc<DnaDef>,
}

impl AppValidationWorkspace {
    pub fn new(
        // Writeable because of warrants
        authored_db: DbWrite<DbKindAuthored>,
        dht_db: DbWrite<DbKindDht>,
        dht_db_cache: DhtDbQueryCache,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        _dna_def: Arc<DnaDef>,
    ) -> Self {
        Self {
            authored_db,
            dht_db,
            dht_db_cache,
            cache,
            keystore,
            _dna_def,
        }
    }

    pub async fn validation_workspace(&self) -> AppValidationResult<HostFnWorkspaceRead> {
        Ok(HostFnWorkspace::new(
            self.authored_db.clone().into(),
            self.dht_db.clone().into(),
            self.dht_db_cache.clone(),
            self.cache.clone(),
            self.keystore.clone(),
            None,
        )
        .await?)
    }

    pub fn full_cascade<Network: HolochainP2pDnaT>(&self, network: Network) -> CascadeImpl {
        CascadeImpl::empty()
            .with_authored(self.authored_db.clone().into())
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
