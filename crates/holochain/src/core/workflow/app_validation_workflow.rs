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
//! were authored (op order). Validating one op
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
//! used. Ops that do not relate to a specific entry or link like [`ChainOp::AgentActivity`]
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
use crate::conductor::entry_def_store::get_entry_def;
use crate::conductor::Conductor;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::ribosome::guest_callback::validate::ValidateHostAccess;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::Ribosome;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::validation::OutcomeOrError;
use crate::core::SysValidationError;
use crate::core::SysValidationResult;
use crate::core::ValidationOutcome;
pub use error::*;
use holo_hash::DhtOpHash;
use holo_hash::HoloHashed;
use holochain_cascade::Cascade;
use holochain_cascade::CascadeImpl;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::{NetworkRequestOptions as NetworkGetOptions, NetworkRequestOptions};
use holochain_p2p::DynHolochainP2pDna;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_state::prelude::*;
// App validation dispatches the v2 `Op` to the wasm `validate` callback; the
// bare `Op` name (and its variant structs) otherwise resolves to the legacy op
// module via the glob imports above. The op pipeline itself is v2-native:
// `ChainOp`/`DhtOp`/`OpEntry` similarly shadow the legacy re-exports.
use holochain_types::dht_v2::{ChainOp, DhtOp, OpEntry};
use holochain_zome_types::dependencies::holochain_integrity_types::dht_v2::{
    Op, RegisterAgentActivity, RegisterCreateLink, RegisterDelete, RegisterDeleteLink,
    RegisterUpdate, StoreEntry, StoreRecord,
};
use holochain_zome_types::dht_v2::{
    to_legacy_signed_action, ActionData, CreateData, CreateLinkData, DeleteData, DeleteLinkData,
    SignedActionHashed, UpdateData,
};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tracing::*;
pub use types::Outcome;

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
    ))
)]
#[allow(clippy::too_many_arguments)]
pub async fn app_validation_workflow(
    dna_hash: Arc<DnaHash>,
    workspace: Arc<AppValidationWorkspace>,
    trigger_integration: TriggerSender,
    trigger_publish: TriggerSender,
    conductor_handle: ConductorHandle,
    network: DynHolochainP2pDna,
    representative_agent: AgentPubKey,
) -> WorkflowResult<WorkComplete> {
    let outcome_summary = app_validation_workflow_inner(
        dna_hash,
        workspace,
        conductor_handle,
        network,
        representative_agent,
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
    network: DynHolochainP2pDna,
    _representative_agent: AgentPubKey,
) -> WorkflowResult<OutcomeSummary> {
    let sorted_dht_ops = workspace
        .dht_store
        .as_read()
        .ops_pending_app_validation(10_000)
        .await?;
    let num_ops_to_validate = sorted_dht_ops.len();

    let cascade = Arc::new(workspace.full_cascade(network.clone()));
    let accepted_ops = Arc::new(AtomicUsize::new(0));
    let awaiting_ops = Arc::new(AtomicUsize::new(0));
    let rejected_ops = Arc::new(AtomicUsize::new(0));
    let failed_ops = Arc::new(Mutex::new(HashSet::new()));
    let mut agent_activity_ops = vec![];
    // Locally-validated warrant ops, self-published into the DhtStore.
    let mut warrant_ops_vec: Vec<holochain_types::dht_v2::DhtOpHashed> = vec![];
    let mut app_validation_outcomes: Vec<(DhtOpHash, AppOutcome)> = vec![];
    // Track action hashes already warranted in this batch to avoid creating duplicate
    // warrants for the same action. Multiple op types (StoreRecord, StoreEntry,
    // RegisterAgentActivity) can share the same action, and without this deduplication
    // all of them would trigger a separate warrant when processed in the same run.
    let mut warranted_in_batch = std::collections::HashSet::<holo_hash::ActionHash>::new();

    #[cfg(feature = "test_utils")]
    let disable_warrant_issuance = conductor
        .config
        .conductor_tuning_params()
        .disable_warrant_issuance;
    #[cfg(not(feature = "test_utils"))]
    let disable_warrant_issuance = false;

    // Validate ops sequentially
    for sorted_dht_op in sorted_dht_ops.into_iter() {
        let (dht_op, dht_op_hash) = sorted_dht_op.into_inner();

        let chain_op = match dht_op {
            DhtOp::ChainOp(chain_op) => chain_op,
            _ => unreachable!("warrant ops are never sent to app validation"),
        };

        let op_type = chain_op.op_type();
        let action = chain_op.signed_action().data();

        // If this is agent activity, track it for the cache.
        let agent_activity_op = matches!(op_type, ChainOpType::RegisterAgentActivity)
            .then(|| (action.author().clone(), action.action_seq()));

        // Validate this op
        let validation_outcome = match chain_op_to_op(*chain_op.clone(), cascade.clone()).await {
            Ok(op) => {
                validate_op_outer(
                    dna_hash.clone(),
                    &op,
                    &conductor,
                    &workspace,
                    network.clone(),
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

        match validation_outcome {
            Ok(outcome) => {
                // Collect all agent activity.
                if let Some(agent_activity_op) = agent_activity_op {
                    // If the activity is accepted or rejected then it's ready to integrate.
                    if matches!(&outcome, Outcome::Accepted | Outcome::Rejected(_)) {
                        agent_activity_ops.push(agent_activity_op);
                    }
                }
                if let Outcome::Rejected(_) = &outcome {
                    warn!(?outcome, ?chain_op, "DhtOp has failed app validation");
                } else if let Outcome::AwaitingDeps(_) = &outcome {
                    debug!(?outcome, ?chain_op, "DhtOp cannot be app validated yet");
                }

                if let Outcome::Rejected(reason) = &outcome {
                    let action_hash = chain_op.signed_action().data().to_hash();

                    let issue_warrant = if warranted_in_batch.contains(&action_hash) {
                        tracing::trace!(
                            "Op {} action is already being warranted in this batch, skipping",
                            dht_op_hash
                        );
                        false
                    } else {
                        match workspace
                            .dht_store
                            .as_read()
                            .is_action_warranted_as_invalid(
                                &action_hash,
                                chain_op.signed_action().data().author(),
                            )
                            .await
                        {
                            Ok(true) => {
                                tracing::trace!(
                                    "Op {} is already warranted, not issuing a new warrant",
                                    dht_op_hash
                                );
                                false
                            }
                            Ok(false) => {
                                // Not warranted yet, should issue a warrant.
                                true
                            }
                            Err(e) => {
                                tracing::error!(error = ?e, "Error checking if op is warranted");
                                false
                            }
                        }
                    };

                    if disable_warrant_issuance {
                        tracing::warn!("Warrant issuance disabled - skipping issuing a warrant");
                    } else if !issue_warrant {
                        tracing::trace!("Not issuing a warrant for op {}", dht_op_hash);
                    } else {
                        warranted_in_batch.insert(action_hash);
                        let keystore = conductor.keystore();
                        let warrant_op =
                            crate::core::workflow::sys_validation_workflow::make_invalid_chain_warrant_op(
                                keystore,
                                _representative_agent.clone(),
                                &chain_op,
                                reason,
                            )
                            .await?;

                        warrant_ops_vec.push(warrant_op);
                    }
                }

                match outcome {
                    Outcome::Accepted => {
                        accepted_ops.fetch_add(1, Ordering::SeqCst);
                        app_validation_outcomes.push((dht_op_hash, AppOutcome::Accepted));
                    }
                    Outcome::AwaitingDeps(_) => {
                        // Status stays NULL; nothing to record.
                        awaiting_ops.fetch_add(1, Ordering::SeqCst);
                    }
                    Outcome::Rejected(_) => {
                        rejected_ops.fetch_add(1, Ordering::SeqCst);
                        tracing::info!(
                            "Received invalid op. The op author will be blocked. Op: {chain_op:?}"
                        );
                        app_validation_outcomes.push((dht_op_hash, AppOutcome::Rejected));
                    }
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

    let accepted_ops = accepted_ops.load(Ordering::SeqCst);
    let awaiting_ops = awaiting_ops.load(Ordering::SeqCst);
    let rejected_ops = rejected_ops.load(Ordering::SeqCst);
    let warranted_ops = warrant_ops_vec.len();
    let ops_validated = accepted_ops + rejected_ops;
    let failed_ops = Arc::try_unwrap(failed_ops)
        .expect("must be only reference")
        .into_inner();
    tracing::info!("{ops_validated} out of {num_ops_to_validate} validated: {accepted_ops} accepted, {awaiting_ops} awaiting deps, {rejected_ops} rejected, failed ops {failed_ops:?}.");

    // Record app validation outcomes into the DhtStore.
    if !app_validation_outcomes.is_empty() {
        workspace
            .dht_store
            .record_app_validation_outcomes(app_validation_outcomes)
            .await?;
    }

    // "self-publish" locally-validated warrant ops into the DhtStore as if they
    // were published to us by another node.
    if !warrant_ops_vec.is_empty() {
        workspace
            .dht_store
            .record_locally_validated_warrants(warrant_ops_vec)
            .await?;
    }

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
    // Register agent activity doesn't store the entry so we need to
    // save it so we can reconstruct the record later.
    if matches!(op_type, ChainOpType::RegisterAgentActivity) {
        hidden_entry = entry.take().or(hidden_entry);
    }

    let dht_op_hash =
        holochain_types::dht_v2::ChainOpUniqueForm::op_hash(op_type, &sah.hashed.content);

    let op = match op_type {
        ChainOpType::StoreRecord => {
            let visibility = sah.hashed.content.entry_visibility().copied();
            Op::StoreRecord(StoreRecord {
                record: Record::new(sah, RecordEntry::new(visibility.as_ref(), entry)),
            })
        }
        ChainOpType::StoreEntry => {
            let entry = entry.ok_or_else(|| {
                AppValidationError::DhtOpError(DhtOpError::ActionWithoutEntry(Box::new(
                    to_legacy_signed_action(&sah).action().clone(),
                )))
            })?;
            Op::StoreEntry(StoreEntry { action: sah, entry })
        }
        ChainOpType::RegisterAgentActivity => Op::RegisterAgentActivity(RegisterAgentActivity {
            action: sah,
            cached_entry: entry,
        }),
        ChainOpType::RegisterUpdatedContent | ChainOpType::RegisterUpdatedRecord => {
            Op::RegisterUpdate(RegisterUpdate {
                update: sah,
                new_entry: entry,
            })
        }
        ChainOpType::RegisterDeletedBy | ChainOpType::RegisterDeletedEntryAction => {
            Op::RegisterDelete(RegisterDelete { delete: sah })
        }
        ChainOpType::RegisterAddLink => {
            Op::RegisterCreateLink(RegisterCreateLink { create_link: sah })
        }
        ChainOpType::RegisterRemoveLink => {
            let link_add_address = match &sah.hashed.content.data {
                ActionData::DeleteLink(DeleteLinkData {
                    link_add_address, ..
                }) => link_add_address.clone(),
                _ => {
                    let legacy_action = to_legacy_signed_action(&sah).action().clone();
                    return Err(AppValidationError::DhtOpError(DhtOpError::OpActionMismatch(
                        op_type,
                        (&legacy_action).into(),
                    ))
                    .into());
                }
            };
            let create_link = cascade
                .retrieve_action(link_add_address.clone(), Default::default())
                .await?
                .map(|(sh, _)| sh.hashed.content)
                .ok_or_else(|| Outcome::awaiting(&link_add_address))?;
            Op::RegisterDeleteLink(RegisterDeleteLink {
                delete_link: sah,
                create_link,
            })
        }
    };

    Ok((op, dht_op_hash, hidden_entry))
}

/// The legacy `ActionType` a v2 action projects to, for the
/// `DhtOpError::OpActionMismatch` diagnostic below (which is defined against the
/// legacy discriminant — a different enum from v2's own `ActionType`).
/// Error-message-only: never used to decide validation outcomes.
fn legacy_action_type(action: &Action) -> ActionType {
    let hashed = HoloHashed::from_content_sync(action.clone());
    let sah = SignedActionHashed::with_presigned(hashed, Signature::from([0u8; 64]));
    let legacy_action = to_legacy_signed_action(&sah).action().clone();
    (&legacy_action).into()
}

/// Build the v2 `Op` (the wasm `validate` callback's input) from a sys-validated
/// `ChainOp`. Sys validation has already rejected any op whose action doesn't
/// match its `ChainOp` variant, so the `OpActionMismatch` branches below are
/// defence-in-depth, not an expected path.
async fn chain_op_to_op(chain_op: ChainOp, cascade: Arc<impl Cascade>) -> AppValidationOutcome<Op> {
    let signed_action = chain_op.signed_action().clone();
    let hashed = HoloHashed::from_content_sync(signed_action.data().clone());
    let sah = SignedHashed::with_presigned(hashed, signed_action.signature().clone());
    let action = signed_action.data();

    let op = match chain_op {
        ChainOp::CreateRecord(_, op_entry) => {
            let visibility = action.entry_visibility().copied();
            let entry = match op_entry {
                OpEntry::Present(entry) => Some(entry),
                OpEntry::Hidden | OpEntry::ActionOnly => None,
            };
            Op::StoreRecord(StoreRecord {
                record: Record::new(sah, RecordEntry::new(visibility.as_ref(), entry)),
            })
        }
        ChainOp::CreateEntry(_, op_entry) => {
            let entry = match op_entry {
                OpEntry::Present(entry) => entry,
                OpEntry::Hidden | OpEntry::ActionOnly => {
                    return Err(
                        AppValidationError::DhtOpError(DhtOpError::ActionWithoutEntry(Box::new(
                            to_legacy_signed_action(&sah).action().clone(),
                        )))
                        .into(),
                    );
                }
            };
            Op::StoreEntry(StoreEntry { action: sah, entry })
        }
        ChainOp::AgentActivity(_) => Op::RegisterAgentActivity(RegisterAgentActivity {
            action: sah,
            cached_entry: None,
        }),
        ChainOp::UpdateEntry(_, op_entry) | ChainOp::UpdateRecord(_, op_entry) => {
            let ActionData::Update(update) = &action.data else {
                return Err(AppValidationError::DhtOpError(DhtOpError::OpActionMismatch(
                    ChainOpType::RegisterUpdatedContent,
                    legacy_action_type(action),
                ))
                .into());
            };
            let entry_visibility = *update.entry_type.visibility();
            let entry_hash = update.entry_hash.clone();
            let entry = match op_entry {
                OpEntry::Present(entry) => Some(entry),
                OpEntry::Hidden | OpEntry::ActionOnly => None,
            };
            let new_entry = match entry_visibility {
                EntryVisibility::Public => match entry {
                    Some(entry) => Some(entry),
                    None => Some(
                        cascade
                            .retrieve_entry(entry_hash.clone(), Default::default())
                            .await?
                            .map(|(e, _)| e.into_content())
                            .ok_or_else(|| Outcome::awaiting(&entry_hash))?,
                    ),
                },
                _ => None,
            };
            Op::RegisterUpdate(RegisterUpdate {
                update: sah,
                new_entry,
            })
        }
        ChainOp::DeleteRecord(_) | ChainOp::DeleteEntry(_) => {
            Op::RegisterDelete(RegisterDelete { delete: sah })
        }
        ChainOp::CreateLink(_) => Op::RegisterCreateLink(RegisterCreateLink { create_link: sah }),
        ChainOp::DeleteLink(_) => {
            let ActionData::DeleteLink(DeleteLinkData {
                link_add_address, ..
            }) = &action.data
            else {
                return Err(AppValidationError::DhtOpError(DhtOpError::OpActionMismatch(
                    ChainOpType::RegisterRemoveLink,
                    legacy_action_type(action),
                ))
                .into());
            };
            let link_add_address = link_add_address.clone();
            let create_link = cascade
                .retrieve_action(link_add_address.clone(), Default::default())
                .await?
                .map(|(sh, _)| sh.hashed.content)
                .ok_or_else(|| Outcome::awaiting(&link_add_address))?;
            Op::RegisterDeleteLink(RegisterDeleteLink {
                delete_link: sah,
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
    network: DynHolochainP2pDna,
) -> AppValidationOutcome<Outcome> {
    // Get the workspace for the validation calls
    let host_fn_workspace = workspace.validation_workspace().await?;

    // Get any Ribosome associated to a cell id with the given dna hash
    // (for app validation we only care about using the correct integrity
    // zomes so it doesn't matter which Ribosome exactly we pick if there are
    // multiple Ribosomes for the same dna hash in the RibosomeStore).
    let ribosome = conductor_handle
        .get_any_ribosome_for_dna_hash(dna_hash.as_ref())
        .map_err(|_| AppValidationError::DnaMissing((*dna_hash).clone()))?;

    validate_op(
        op,
        host_fn_workspace,
        network,
        &ribosome,
        conductor_handle,
        false, // is_inline
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn validate_op(
    op: &Op,
    workspace: HostFnWorkspaceRead,
    network: DynHolochainP2pDna,
    ribosome: &Ribosome,
    conductor_handle: &ConductorHandle,
    is_inline: bool,
) -> AppValidationOutcome<Outcome> {
    check_entry_def(op, &network.dna_hash(), conductor_handle)
        .await
        .map_err(AppValidationError::SysValidationError)?;

    let zomes_to_invoke = get_zomes_to_invoke(op, &workspace, network.clone(), ribosome).await;
    if let Err(OutcomeOrError::Err(err)) = &zomes_to_invoke {
        tracing::error!(?op, ?err, "Error getting zomes to invoke to validate op.");
    };
    let zomes_to_invoke = zomes_to_invoke?;
    let invocation = ValidateInvocation::new(zomes_to_invoke, op)
        .map_err(|e| AppValidationError::RibosomeError(e.into()))?;

    let outcome =
        run_validation_callback(invocation, ribosome, workspace, network, is_inline).await?;

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

    // For entry defs we only care about using the correct integrity
    // zomes so it doesn't matter which Ribosome exactly we pick if there are
    // multiple Ribosomes for the same dna hash in the RibosomeStore.
    let ribosome = conductor
        .get_any_ribosome_for_dna_hash(dna_hash)
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
    network: DynHolochainP2pDna,
    ribosome: &Ribosome,
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
            let action = match &record.action().data {
                ActionData::Delete(DeleteData {
                    deletes_address, ..
                })
                | ActionData::DeleteLink(DeleteLinkData {
                    link_add_address: deletes_address,
                    ..
                }) => {
                    let deleted_action =
                        retrieve_deleted_action(workspace, network, deletes_address).await?;
                    deleted_action.hashed.content.clone()
                }
                _ => record.action().clone(),
            };

            match &action.data {
                ActionData::CreateLink(CreateLinkData { zome_index, .. }) => {
                    get_integrity_zome_from_ribosome(zome_index, ribosome)
                }
                ActionData::Create(CreateData {
                    entry_type: EntryType::App(AppEntryDef { zome_index, .. }),
                    ..
                })
                | ActionData::Update(UpdateData {
                    entry_type: EntryType::App(AppEntryDef { zome_index, .. }),
                    ..
                }) => get_integrity_zome_from_ribosome(zome_index, ribosome),
                _ => Ok(ZomesToInvoke::AllIntegrity),
            }
        }
        Op::StoreEntry(StoreEntry { action, .. }) => match &action.hashed.content.data {
            ActionData::Create(CreateData {
                entry_type: EntryType::App(app_entry_def),
                ..
            })
            | ActionData::Update(UpdateData {
                entry_type: EntryType::App(app_entry_def),
                ..
            }) => get_integrity_zome_from_ribosome(&app_entry_def.zome_index, ribosome),
            _ => Ok(ZomesToInvoke::AllIntegrity),
        },
        Op::RegisterUpdate(RegisterUpdate { update, .. }) => match &update.hashed.content.data {
            ActionData::Update(UpdateData {
                entry_type: EntryType::App(app_entry_def),
                ..
            }) => get_integrity_zome_from_ribosome(&app_entry_def.zome_index, ribosome),
            _ => Ok(ZomesToInvoke::AllIntegrity),
        },
        Op::RegisterDelete(RegisterDelete { delete }) => {
            let deletes_address = match &delete.hashed.content.data {
                ActionData::Delete(DeleteData {
                    deletes_address, ..
                }) => deletes_address,
                // Not expected: `RegisterDelete`'s action data is always `Delete`.
                _ => return Ok(ZomesToInvoke::AllIntegrity),
            };
            let deleted_action =
                retrieve_deleted_action(workspace, network, deletes_address).await?;
            match &deleted_action.hashed.content.data {
                ActionData::Create(CreateData {
                    entry_type: EntryType::App(app_entry_def),
                    ..
                })
                | ActionData::Update(UpdateData {
                    entry_type: EntryType::App(app_entry_def),
                    ..
                }) => get_integrity_zome_from_ribosome(&app_entry_def.zome_index, ribosome),
                _ => Ok(ZomesToInvoke::AllIntegrity),
            }
        }
        Op::RegisterCreateLink(RegisterCreateLink { create_link }) => {
            match &create_link.hashed.content.data {
                ActionData::CreateLink(CreateLinkData { zome_index, .. }) => {
                    get_integrity_zome_from_ribosome(zome_index, ribosome)
                }
                // Not expected: `RegisterCreateLink`'s action data is always `CreateLink`.
                _ => Ok(ZomesToInvoke::AllIntegrity),
            }
        }
        Op::RegisterDeleteLink(RegisterDeleteLink { create_link, .. }) => {
            match &create_link.data {
                ActionData::CreateLink(CreateLinkData { zome_index, .. }) => {
                    get_integrity_zome_from_ribosome(zome_index, ribosome)
                }
                // Not expected: `RegisterDeleteLink::create_link` is always `CreateLink`.
                _ => Ok(ZomesToInvoke::AllIntegrity),
            }
        }
    }
}

async fn retrieve_deleted_action(
    workspace: &HostFnWorkspaceRead,
    network: DynHolochainP2pDna,
    deletes_address: &ActionHash,
) -> AppValidationOutcome<SignedActionHashed> {
    let cascade = CascadeImpl::from_workspace_and_network(workspace, network.clone());
    let (deleted_action, _) = cascade
        .retrieve_action(deletes_address.clone(), NetworkRequestOptions::default())
        .await?
        .ok_or_else(|| Outcome::awaiting(deletes_address))?;
    Ok(deleted_action)
}

fn get_integrity_zome_from_ribosome(
    zome_index: &ZomeIndex,
    ribosome: &Ribosome,
) -> AppValidationOutcome<ZomesToInvoke> {
    let zome = ribosome.get_integrity_zome(zome_index).ok_or_else(|| {
        Outcome::rejected(format!("No integrity zome found with index {zome_index:?}"))
    })?;
    Ok(ZomesToInvoke::OneIntegrity(zome))
}

#[allow(clippy::too_many_arguments)]
async fn run_validation_callback(
    invocation: ValidateInvocation,
    ribosome: &Ribosome,
    workspace: HostFnWorkspaceRead,
    network: DynHolochainP2pDna,
    is_inline: bool,
) -> AppValidationResult<Outcome> {
    let validate_result = ribosome
        .run_validate(
            ValidateHostAccess::new(workspace.clone(), network.clone(), is_inline),
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
                        .fetch_record(hash.clone(), NetworkRequestOptions::must_get_options())
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
                        .must_get_agent_activity(
                            author.clone(),
                            filter,
                            NetworkGetOptions::must_get_options(),
                        )
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
    dht_store: DhtStore,
    keystore: MetaLairClient,
}

impl AppValidationWorkspace {
    pub fn new(dht_store: DhtStore, keystore: MetaLairClient) -> Self {
        Self {
            dht_store,
            keystore,
        }
    }

    pub async fn validation_workspace(&self) -> AppValidationResult<HostFnWorkspaceRead> {
        Ok(HostFnWorkspaceRead::new(self.dht_store.clone(), self.keystore.clone(), None).await?)
    }

    pub fn full_cascade(&self, network: DynHolochainP2pDna) -> CascadeImpl {
        CascadeImpl::empty(self.dht_store.clone()).with_network(network)
    }
}
