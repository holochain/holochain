//! ### The sys validation workflow
//!
//! This workflow runs against all [`DhtOp`]s that are in the DHT database, either coming from the authored database or from other nodes on the network via gossip and publishing.
//!
//! The purpose of the workflow is to make fundamental checks on the integrity of the data being put into the DHT. This ensures that invalid data is not served
//! to other nodes on the network. It also saves hApp developers from having to write these checks themselves since they set the minimum standards that all data
//! should meet regardless of the requirements of a given hApp.
//!
//! #### Validation checks
//!
//! The workflow operates on [`DhtOp`]s which are roughly equivalent to [`Record`]s but catered to the needs of a specific type of Authority.
//! Checks that you can rely on sys validation having performed are:
//! - For a [`DhtOp::StoreRecord`]
//!    - Check that the [`Action`] is either a [`Action::Dna`] at sequence number 0, or has a previous action with sequence number strictly greater than 0.
//!    - If the [`Entry`] is an [`Entry::CounterSign`], then the countersigning session data is mapped to a set of [`Action`]s and each of those actions must be be found locally before this op can progress.
//!    - The [`Action`] must be either a [`Action::Create`] or an [`Action::Update`].
//!    - Run the [store entry checks](#store-entry-checks).
//! - For a [`DhtOp::StoreEntry`]
//!    - If the [`Entry`] is an [`Entry::CounterSign`], then the countersigning session data is mapped to a set of [`Action`]s and each of those actions must be be found locally before this op is accepted.
//!    - Check that the [`Action`] is either a [`Action::Dna`] at sequence number 0, or has a previous action with sequence number strictly greater than 0.
//!    - Run the [store entry checks](#store-entry-checks).
//! - For a [`DhtOp::RegisterAgentActivity`]
//!    - Check that the [`Action`] is either a [`Action::Dna`] at sequence number 0, or has a previous action with sequence number strictly greater than 0.
//!    - If the [`Action`] is a [`Action::Dna`], then verify the contained DNA hash matches the DNA hash that sys validation is being run for.
//!    - Run the [store record checks](#store-record-checks).
//! - For a [`DhtOp::RegisterUpdatedContent`]
//!    - The [`Update::original_action_address`] reference to the [`Action`] being updated must point to an [`Action`] that can be found locally. Once the [`Action`] address has been resolved, the [`Update::original_entry_address`] is checked against the entry address that the referenced [`Action`] specified.
//!    - If there is an [`Entry`], then the [store entry checks](#store-entry-checks) are run.
//! - For a [`DhtOp::RegisterUpdatedRecord`]
//!    - The [`Update::original_action_address`] reference to the [`Action`] being updated must point to an [`Action`] that can be found locally. Once the [`Action`] address has been resolved, the [`Update::original_entry_address`] is checked against the entry address that the referenced [`Action`] specified.
//!    - If there is an [`Entry`], then the [store entry checks](#store-entry-checks) are run.
//! - For a [`DhtOp::RegisterDeletedBy`]
//!    - The [`Delete::deletes_address`] reference to the [`Action`] being deleted must point to an [`Action`] that can be found locally. The action being deleted must be a [`Action::Create`] or [`Action::Update`].
//! - For a [`DhtOp::RegisterDeletedEntryAction`]
//!    - The [`Delete::deletes_address`] reference to the [`Action`] being deleted must point to an [`Action`] that can be found locally. The action being deleted must be a [`Action::Create`] or [`Action::Update`].
//! - For a [`DhtOp::RegisterAddLink`]
//!   - The size of the [`CreateLink::tag`] must be less than or equal to the maximum size that is accepted for this link tag. This is specified in the constant [`MAX_TAG_SIZE`].
//! - For a [`DhtOp::RegisterRemoveLink`]
//!   - The [`DeleteLink::link_add_address`] reference to the [`Action`] of the link being deleted must point to an [`Action`] that can be found locally. That action being deleted must also
//!     be a [`Action::CreateLink`].
//!
//! ##### Store record checks
//!
//! These checks are run when storing a new action for a [`DhtOp`].
//!
//! - Check that the [`Action`] is either a [`Action::Dna`] at sequence number 0, or has a previous action with sequence number strictly greater than 0.
//! - Checks that the author of the current action is the same as the author of the previous action.
//! - Checks that the timestamp of the current action is greater than the timestamp of the previous action.
//! - Checks that the sequence number of the current action is exactly 1 more than the sequence number of the previous action.
//! - Checks that every [`Action::Create`] or [`Action::Update`] of an `AgentPubKey` is preceded by an [`Action::AgentValidationPkg`].
//!
//! ##### Store entry checks
//!
//! These checks are run when storing an entry that is included as part of a [`DhtOp`].
//!
//! - The entry type specified in the [`Action`] must match the entry type specified in the [`Entry`].
//! - The entry hash specified in the [`Action`] must match the entry hash specified in the [`Entry`], which will be hashed as part of the check to obtain a value that is deterministic.
//! - The size of the [`Entry`] must be less than or equal to the maximum size that is accepted for this entry type. This is specified in the constant [`MAX_ENTRY_SIZE`].
//! - If the [`Action`] is an [`Action::Update`], then the [`Update::original_action_address`] reference to the [`Action`] being updated must point to an [`Action`] that can be found locally. Once the [`Action`] address has been resolved, the [`Update::original_entry_address`] is checked against the entry address that the referenced [`Action`] specified.
//! - If the [`Entry`] is an [`Entry::CounterSign`], then the pre-flight response signatures are checked.
//!
//! #### Workflow description
//!
//! - The workflow starts by fetching all the ops that need to be validated from the database. The ops are processed as follows:
//!     - Ops are sorted by [`OpOrder`], to make it more likely that incoming ops will be processed in the order they were created.
//!     - The dependencies of these ops are then concurrently fetched from any of the local databases. Missing dependencies are handled later.
//!     - The [validation checks](#validation-checks) are run for each op.
//!     - For any ops that passed validation, they will be marked as ready for app validation in the database.
//!     - Any ops which were rejected will be marked rejected in the database.
//! - If any ops passed validation, then app validation will be triggered.
//! - For actions that were not found locally, the workflow will then attempt to fetch them from the network.
//! - If any actions that were missing are found on the network, then sys validation is re-triggered to see if the newly fetched actions allow any outstanding ops to pass validation.
//! - If fewer actions were fetched from the network than there were actions missing, then the workflow will sleep for a short time before re-triggering itself.
//! - Once all ops have an outcome, the workflow is complete and will wait to be triggered again by new incoming ops.
//!

use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::sys_validate::*;
use crate::core::validation::*;
use crate::core::workflow::error::WorkflowResult;
use futures::FutureExt;
use futures::StreamExt;
use holo_hash::DhtOpHash;
use holochain_cascade::Cascade;
use holochain_cascade::CascadeImpl;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use parking_lot::Mutex;
use rusqlite::Transaction;
use std::convert::TryInto;
use std::sync::Arc;
use std::time::Duration;
use tracing::*;
use types::Outcome;

use self::validation_deps::ValidationDependencies;
use self::validation_deps::ValidationDependencyState;

pub mod types;

pub mod validation_deps;
pub mod validation_query;

#[cfg(test)]
mod chain_test;
#[cfg(test)]
mod test_ideas;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod unit_tests;
#[cfg(test)]
mod validate_op_tests;

/// The sys validation worfklow. It is described in the module level documentation.
#[instrument(skip(
    workspace,
    current_validation_dependencies,
    trigger_app_validation,
    trigger_self,
    network,
    config
))]
pub async fn sys_validation_workflow<Network: HolochainP2pDnaT + Clone + 'static>(
    workspace: Arc<SysValidationWorkspace>,
    current_validation_dependencies: Arc<Mutex<ValidationDependencies>>,
    trigger_app_validation: TriggerSender,
    trigger_self: TriggerSender,
    network: Network,
    config: Arc<ConductorConfig>,
) -> WorkflowResult<WorkComplete> {
    // Run the actual sys validation using data we have locally
    let outcome_summary = sys_validation_workflow_inner(
        workspace.clone(),
        current_validation_dependencies.clone(),
        config,
    )
    .await?;

    // trigger app validation to process any ops that have been processed so far
    if outcome_summary.accepted > 0 {
        tracing::debug!("Sys validation accepted {} ops", outcome_summary.accepted);

        trigger_app_validation.trigger(&"sys_validation_workflow");
    }

    // Now go to the network to try to fetch missing dependencies
    let network_cascade = Arc::new(workspace.network_and_cache_cascade(network));
    let missing_action_hashes = current_validation_dependencies.lock().get_missing_hashes();
    let num_fetched: usize = futures::stream::iter(missing_action_hashes.into_iter().map(|hash| {
        let network_cascade = network_cascade.clone();
        let current_validation_dependencies = current_validation_dependencies.clone();
        async move {
            match network_cascade
                .retrieve_action(hash, Default::default())
                .await
            {
                Ok(Some((action, source))) => {
                    let mut deps = current_validation_dependencies.lock();

                    // If the source was local then that means some other fetch has put this action into the cache,
                    // that's fine we'll just grab it here.
                    if deps.insert(action, source) {
                        1
                    } else {
                        0
                    }
                }
                Ok(None) => {
                    // This is fine, we didn't find it on the network, so we'll have to try again.
                    // TODO This will hit the network again fairly quickly if sys validation is triggered again soon.
                    //      It would be more efficient to wait a bit before trying again.
                    0
                }
                Err(e) => {
                    tracing::error!(error = ?e, "Error fetching missing dependency");
                    0
                }
            }
        }
        .boxed()
    }))
    .buffer_unordered(10)
    .collect::<Vec<usize>>()
    .await
    .into_iter()
    .sum();

    if outcome_summary.missing > 0 {
        tracing::debug!(
            "Fetched {}/{} missing dependencies from the network",
            num_fetched,
            outcome_summary.missing
        );
    }

    if num_fetched > 0 {
        // If we fetched anything then we can re-run sys validation
        trigger_self.trigger(&"sys_validation_workflow");
    }

    if num_fetched < outcome_summary.missing {
        tracing::info!(
            "Sys validation sleeping for {:?}",
            workspace.sys_validation_retry_delay
        );
        Ok(WorkComplete::Incomplete(Some(
            workspace.sys_validation_retry_delay,
        )))
    } else {
        Ok(WorkComplete::Complete)
    }
}

async fn sys_validation_workflow_inner(
    workspace: Arc<SysValidationWorkspace>,
    current_validation_dependencies: Arc<Mutex<ValidationDependencies>>,
    config: Arc<ConductorConfig>,
) -> WorkflowResult<OutcomeSummary> {
    let db = workspace.dht_db.clone();
    let mut sorted_ops = validation_query::get_ops_to_sys_validate(&db).await?;
    sorted_ops.sort_by_cached_key(|op| OpOrder::new(op.get_type(), op.timestamp()));
    let sleuth_id = config.sleuth_id();

    // Forget what dependencies are currently in use
    current_validation_dependencies.lock().clear_retained_deps();

    if sorted_ops.is_empty() {
        tracing::trace!(
            "Skipping sys_validation_workflow because there are no ops to be validated"
        );

        // If there's nothing to validate then we can clear the dependencies and save some memory.
        current_validation_dependencies.lock().purge_held_deps();

        return Ok(OutcomeSummary::new());
    }

    let num_ops_to_validate = sorted_ops.len();
    tracing::debug!("Sys validating {} ops", num_ops_to_validate);

    let cascade = Arc::new(workspace.local_cascade());
    let dna_def = DnaDefHashed::from_content_sync((*workspace.dna_def()).clone());

    retrieve_previous_actions_for_ops(
        current_validation_dependencies.clone(),
        cascade.clone(),
        sorted_ops.clone().into_iter(),
    )
    .await;

    // Now drop all the dependencies that we didn't just try to access while searching the current set of ops to validate.
    current_validation_dependencies.lock().purge_held_deps();

    let mut validation_outcomes = Vec::with_capacity(sorted_ops.len());
    for hashed_op in sorted_ops {
        let (op, op_hash) = hashed_op.into_inner();
        let op_type = op.get_type();
        let action = op.action();

        // This is an optimization to skip app validation and integration for ops that are
        // rejected and don't have dependencies.
        let dependency = op_type.sys_validation_dependency(&action);

        // Note that this is async only because of the signature checks done during countersigning.
        // In most cases this will be a fast synchronous call.
        let r = validate_op(&op, &dna_def, current_validation_dependencies.clone()).await;

        match r {
            Ok(outcome) => validation_outcomes.push((op_hash, outcome, dependency)),
            Err(e) => {
                tracing::error!(error = ?e, "Error validating op");
            }
        }
    }

    let summary: OutcomeSummary = workspace
        .dht_db
        .write_async(move |txn| {
            let mut summary = OutcomeSummary::default();
            for (op_hash, outcome, dependency) in validation_outcomes {
                match outcome {
                    Outcome::Accepted => {
                        summary.accepted += 1;
                        put_validation_limbo(txn, &op_hash, ValidationStage::SysValidated)?;
                        aitia::trace!(&hc_sleuth::Event::SysValidated {
                            by: sleuth_id.clone(),
                            op: op_hash
                        });
                    }
                    Outcome::MissingDhtDep(missing_dep) => {
                        summary.missing += 1;
                        let status = ValidationStage::AwaitingSysDeps(missing_dep);
                        put_validation_limbo(txn, &op_hash, status)?;
                    }
                    Outcome::Rejected(_) => {
                        summary.rejected += 1;
                        if dependency.is_none() {
                            put_integrated(txn, &op_hash, ValidationStatus::Rejected)?;
                        } else {
                            put_integration_limbo(txn, &op_hash, ValidationStatus::Rejected)?;
                        }
                    }
                }
            }
            WorkflowResult::Ok(summary)
        })
        .await?;

    tracing::debug!(
        ?summary,
        ?num_ops_to_validate,
        "Finished sys validation workflow"
    );

    Ok(summary)
}

async fn retrieve_actions(
    current_validation_dependencies: Arc<Mutex<ValidationDependencies>>,
    cascade: Arc<impl Cascade + Send + Sync>,
    action_hashes: impl Iterator<Item = ActionHash>,
) {
    let action_fetches = action_hashes
        .filter(|hash| !current_validation_dependencies.lock().has(hash))
        .map(|h| {
            // For each previous action that will be needed for validation, map the action to a fetch Action for its hash
            let cascade = cascade.clone();
            async move {
                let fetched = cascade.retrieve_action(h.clone(), Default::default()).await;
                tracing::trace!(hash = ?h, fetched = ?fetched, "Fetched action for validation");
                (h, fetched)
            }
            .boxed()
        });

    let new_deps: ValidationDependencies = futures::future::join_all(action_fetches)
        .await
        .into_iter()
        .filter_map(|r| {
            // Filter out errors, preparing the rest to be put into a HashMap for easy access.
            match r {
                (hash, Ok(Some((signed_action, source)))) => {
                    Some((hash, (signed_action, source).into()))
                }
                (hash, Ok(None)) => {
                    Some((hash, ValidationDependencyState::new(None)))
                },
                (hash, Err(e)) => {
                    tracing::error!(error = ?e, action_hash = ?hash, "Error retrieving prev action");
                    None
                }
            }
        })
        .collect();

    current_validation_dependencies.lock().merge(new_deps);
}

fn get_dependency_hashes_from_actions(actions: impl Iterator<Item = Action>) -> Vec<ActionHash> {
    actions
        .flat_map(|action| {
            vec![
                match action.prev_action().cloned() {
                    None => None,
                    hash => hash,
                },
                match action {
                    Action::Update(action) => Some(action.original_action_address),
                    Action::Delete(action) => Some(action.deletes_address),
                    Action::DeleteLink(action) => Some(action.link_add_address),
                    _ => None,
                },
            ]
            .into_iter()
            .flatten()
        })
        .collect()
}

/// Examine the list of provided actions and create a list of actions which are sys validation dependencies for those actions.
/// The actions are merged into `current_validation_dependencies`.
async fn fetch_previous_actions(
    current_validation_dependencies: Arc<Mutex<ValidationDependencies>>,
    cascade: Arc<impl Cascade + Send + Sync>,
    actions: impl Iterator<Item = Action>,
) {
    retrieve_actions(
        current_validation_dependencies,
        cascade,
        get_dependency_hashes_from_actions(actions).into_iter(),
    )
    .await;
}

fn get_dependency_hashes_from_ops(ops: impl Iterator<Item = DhtOpHashed>) -> Vec<ActionHash> {
    ops.into_iter()
        .filter_map(|op| {
            // For each previous action that will be needed for validation, map the action to a fetch Record for its hash
            match &op.content {
                DhtOp::StoreRecord(_, action, entry) => {
                    let mut actions = match entry {
                        RecordEntry::Present(entry @ Entry::CounterSign(session_data, _)) => {
                            // Discard errors here because we'll check later whether the input is valid. If it's not then it
                            // won't matter that we've skipped fetching deps for it
                            if let Ok(entry_rate_weight) = action_to_entry_rate_weight(action) {
                                make_action_set_for_session_data(
                                    entry_rate_weight,
                                    entry,
                                    session_data,
                                )
                                .unwrap_or_else(|_| vec![])
                                .into_iter()
                                .map(|action| action.into_hash())
                                .collect::<Vec<_>>()
                            } else {
                                vec![]
                            }
                        }
                        _ => vec![],
                    };

                    if let Action::Update(update) = action {
                        actions.push(update.original_action_address.clone());
                    }
                    Some(actions)
                }
                DhtOp::StoreEntry(_, action, entry) => {
                    let mut actions = match entry {
                        Entry::CounterSign(session_data, _) => {
                            // Discard errors here because we'll check later whether the input is valid. If it's not then it
                            // won't matter that we've skipped fetching deps for it
                            make_action_set_for_session_data(
                                new_entry_action_to_entry_rate_weight(action),
                                entry,
                                session_data,
                            )
                            .unwrap_or_else(|_| vec![])
                            .into_iter()
                            .map(|action| action.into_hash())
                            .collect::<Vec<_>>()
                        }
                        _ => vec![],
                    };

                    if let NewEntryAction::Update(update) = action {
                        actions.push(update.original_action_address.clone());
                    }
                    Some(actions)
                }
                DhtOp::RegisterAgentActivity(_, action) => action
                    .prev_action()
                    .map(|action| vec![action.as_hash().clone()]),
                DhtOp::RegisterUpdatedContent(_, action, _) => {
                    Some(vec![action.original_action_address.clone()])
                }
                DhtOp::RegisterUpdatedRecord(_, action, _) => {
                    Some(vec![action.original_action_address.clone()])
                }
                DhtOp::RegisterDeletedBy(_, action) => Some(vec![action.deletes_address.clone()]),
                DhtOp::RegisterDeletedEntryAction(_, action) => {
                    Some(vec![action.deletes_address.clone()])
                }
                DhtOp::RegisterRemoveLink(_, action) => Some(vec![action.link_add_address.clone()]),
                _ => None,
            }
        })
        .flatten()
        .collect()
}

/// Examine the list of provided ops and create a list of actions which are sys validation dependencies for those ops.
/// The actions are merged into `current_validation_dependencies`.
async fn retrieve_previous_actions_for_ops(
    current_validation_dependencies: Arc<Mutex<ValidationDependencies>>,
    cascade: Arc<impl Cascade + Send + Sync>,
    ops: impl Iterator<Item = DhtOpHashed>,
) {
    retrieve_actions(
        current_validation_dependencies,
        cascade,
        get_dependency_hashes_from_ops(ops).into_iter(),
    )
    .await;
}

/// Validate a single DhtOp, using the supplied Cascade to draw dependencies from
pub(crate) async fn validate_op(
    op: &DhtOp,
    dna_def: &DnaDefHashed,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> WorkflowResult<Outcome> {
    match validate_op_inner(op, dna_def, validation_dependencies).await {
        Ok(_) => Ok(Outcome::Accepted),
        // Handle the errors that result in pending or awaiting deps
        Err(SysValidationError::ValidationOutcome(e)) => {
            match e {
                // This is expected if the dependency isn't held locally and needs to be fetched from the network
                // so downgrade the logging to trace.
                ValidationOutcome::DepMissingFromDht(_) => {
                    tracing::trace!(
                        msg = "DhtOp has a missing dependency",
                        ?op,
                        error = ?e,
                        error_msg = %e
                    );
                }
                _ => {
                    info!(
                        msg = "DhtOp did not pass system validation. (If rejected, a warning will follow.)",
                        ?op,
                        error = ?e,
                        error_msg = %e
                    );
                }
            }
            let outcome = handle_failed(&e);
            if let Outcome::Rejected(_) = outcome {
                warn!(msg = "DhtOp was rejected during system validation.", ?op, error = ?e, error_msg = %e)
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
fn handle_failed(error: &ValidationOutcome) -> Outcome {
    use Outcome::*;
    match error {
        ValidationOutcome::Counterfeit(_, _) => {
            unreachable!("Counterfeit ops are dropped before sys validation")
        }
        ValidationOutcome::DepMissingFromDht(dep) => MissingDhtDep(dep.clone()),
        reason => Rejected(reason.to_string()),
    }
}

fn action_to_entry_rate_weight(action: &Action) -> SysValidationResult<EntryRateWeight> {
    action
        .entry_rate_data()
        .ok_or_else(|| SysValidationError::NonEntryAction(action.clone()))
}

fn new_entry_action_to_entry_rate_weight(action: &NewEntryAction) -> EntryRateWeight {
    match action {
        NewEntryAction::Create(h) => h.weight.clone(),
        NewEntryAction::Update(h) => h.weight.clone(),
    }
}

fn make_action_set_for_session_data(
    entry_rate_weight: EntryRateWeight,
    entry: &Entry,
    session_data: &CounterSigningSessionData,
) -> SysValidationResult<Vec<ActionHash>> {
    let entry_hash = EntryHash::with_data_sync(entry);
    Ok(session_data
        .build_action_set(entry_hash, entry_rate_weight)?
        .into_iter()
        .map(|action| ActionHash::with_data_sync(&action))
        .collect())
}

async fn validate_op_inner(
    op: &DhtOp,
    dna_def: &DnaDefHashed,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> SysValidationResult<()> {
    check_entry_visibility(op)?;
    match op {
        DhtOp::StoreRecord(_, action, entry) => {
            check_prev_action(action)?;
            if let Some(entry) = entry.as_option() {
                // Retrieve for all other actions on countersigned entry.
                if let Entry::CounterSign(session_data, _) = entry {
                    for action_hash in make_action_set_for_session_data(
                        action_to_entry_rate_weight(action)?,
                        entry,
                        session_data,
                    )? {
                        // Just require that we are holding all the other actions
                        let mut validation_dependencies = validation_dependencies.lock();
                        validation_dependencies
                            .get(&action_hash)
                            .and_then(|s| s.as_action())
                            .ok_or_else(|| {
                                ValidationOutcome::DepMissingFromDht(action_hash.clone().into())
                            })?;
                    }
                }
                // Has to be async because of signature checks being async
                store_entry(
                    (action)
                        .try_into()
                        .map_err(|_| ValidationOutcome::NotNewEntry(action.clone()))?,
                    entry,
                    validation_dependencies,
                )
                .await?;
            }
            Ok(())
        }
        DhtOp::StoreEntry(_, action, entry) => {
            // Check and hold for all other actions on countersigned entry.
            if let Entry::CounterSign(session_data, _) = entry {
                for action_hash in make_action_set_for_session_data(
                    new_entry_action_to_entry_rate_weight(action),
                    entry,
                    session_data,
                )? {
                    // Just require that we are holding all the other actions
                    let mut validation_dependencies = validation_dependencies.lock();
                    validation_dependencies
                        .get(&action_hash)
                        .and_then(|s| s.as_action())
                        .ok_or_else(|| {
                            ValidationOutcome::DepMissingFromDht(action_hash.clone().into())
                        })?;
                }
            }

            check_prev_action(&action.clone().into())?;
            store_entry(action.into(), entry, validation_dependencies.clone()).await
        }
        DhtOp::RegisterAgentActivity(_, action) => {
            register_agent_activity(action, validation_dependencies.clone(), dna_def)?;
            store_record(action, validation_dependencies)
        }
        DhtOp::RegisterUpdatedContent(_, action, entry) => {
            register_updated_content(action, validation_dependencies.clone())?;
            if let Some(entry) = entry.as_option() {
                store_entry(
                    NewEntryActionRef::Update(action),
                    entry,
                    validation_dependencies,
                )
                .await?;
            }

            Ok(())
        }
        DhtOp::RegisterUpdatedRecord(_, action, entry) => {
            register_updated_record(action, validation_dependencies.clone())?;
            if let Some(entry) = entry.as_option() {
                store_entry(
                    NewEntryActionRef::Update(action),
                    entry,
                    validation_dependencies,
                )
                .await?;
            }

            Ok(())
        }
        DhtOp::RegisterDeletedBy(_, action) => register_deleted_by(action, validation_dependencies),
        DhtOp::RegisterDeletedEntryAction(_, action) => {
            register_deleted_entry_action(action, validation_dependencies)
        }
        DhtOp::RegisterAddLink(_, action) => register_add_link(action),
        DhtOp::RegisterRemoveLink(_, action) => {
            register_delete_link(action, validation_dependencies)
        }
    }
}

/// Run system validation for a single [`Record`] instead of the usual [`DhtOp`] input for the system validation workflow.
/// It is expected that the provided cascade will include a network so that dependencies which we either do not hold yet, or
/// should not hold, can be fetched and cached for use in validation.
///
/// Note that the conditions on the action being validated are slightly stronger than the usual system validation workflow. This is because
/// it is intended to be used for validation of records which have been authored locally so we should always be able to check the previous action.
pub async fn sys_validate_record(
    record: &Record,
    cascade: Arc<impl Cascade + Send + Sync>,
) -> SysValidationOutcome<()> {
    match sys_validate_record_inner(record, cascade).await {
        // Validation succeeded
        Ok(_) => Ok(()),
        // Validation failed so exit with that outcome
        Err(SysValidationError::ValidationOutcome(validation_outcome)) => {
            error!(
                msg = "Direct validation failed",
                ?validation_outcome,
                ?record,
            );
            validation_outcome.into_outcome()
        }
        // An error occurred so return it
        Err(e) => Err(OutcomeOrError::Err(e)),
    }
}

async fn sys_validate_record_inner(
    record: &Record,
    cascade: Arc<impl Cascade + Send + Sync>,
) -> SysValidationResult<()> {
    let signature = record.signature();
    let action = record.action();
    let maybe_entry = record.entry().as_option();
    counterfeit_check(signature, action).await?;

    async fn validate(
        action: &Action,
        maybe_entry: Option<&Entry>,
        cascade: Arc<impl Cascade + Send + Sync>,
    ) -> SysValidationResult<()> {
        let validation_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));
        fetch_previous_actions(
            validation_dependencies.clone(),
            cascade.clone(),
            vec![action.clone()].into_iter(),
        )
        .await;

        store_record(action, validation_dependencies.clone())?;
        if let Some(maybe_entry) = maybe_entry {
            store_entry(
                action
                    .try_into()
                    .map_err(|_| ValidationOutcome::NotNewEntry(action.clone()))?,
                maybe_entry,
                validation_dependencies.clone(),
            )
            .await?;
        }
        match action {
            Action::Update(action) => {
                register_updated_content(action, validation_dependencies.clone())
            }
            Action::Delete(action) => {
                register_deleted_entry_action(action, validation_dependencies.clone())
            }
            Action::CreateLink(action) => register_add_link(action),
            Action::DeleteLink(action) => {
                register_delete_link(action, validation_dependencies.clone())
            }
            _ => Ok(()),
        }
    }

    match maybe_entry {
        Some(Entry::CounterSign(session, _)) => {
            if let Some(weight) = action.entry_rate_data() {
                let entry_hash = EntryHash::with_data_sync(maybe_entry.unwrap());
                for action in session.build_action_set(entry_hash, weight)? {
                    validate(&action, maybe_entry, cascade.clone()).await?;
                }
                Ok(())
            } else {
                tracing::error!("Got countersigning entry without rate assigned. This should be impossible. But, let's see what happens.");
                validate(action, maybe_entry, cascade.clone()).await
            }
        }
        _ => validate(action, maybe_entry, cascade).await,
    }
}

/// Check if the op has valid signature and author.
/// Ops that fail this check should be dropped.
pub async fn counterfeit_check(signature: &Signature, action: &Action) -> SysValidationResult<()> {
    verify_action_signature(signature, action).await?;
    author_key_is_valid(action.author()).await?;
    Ok(())
}

fn register_agent_activity(
    action: &Action,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
    dna_def: &DnaDefHashed,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let prev_action_hash = action.prev_action();

    // Checks
    check_prev_action(action)?;
    check_valid_if_dna(action, dna_def)?;
    if let Some(prev_action_hash) = prev_action_hash {
        // Just make sure we have the dependency and if not then don't mark this action as valid yet
        let mut validation_dependencies = validation_dependencies.lock();
        validation_dependencies
            .get(prev_action_hash)
            .and_then(|s| s.as_action())
            .ok_or_else(|| ValidationOutcome::DepMissingFromDht(prev_action_hash.clone().into()))?;
    }

    Ok(())
}

fn store_record(
    action: &Action,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let prev_action_hash = action.prev_action();

    // Checks
    check_prev_action(action)?;
    if let Some(prev_action_hash) = prev_action_hash {
        let mut validation_dependencies = validation_dependencies.lock();
        let prev_action = validation_dependencies
            .get(prev_action_hash)
            .and_then(|s| s.as_action())
            .ok_or_else(|| ValidationOutcome::DepMissingFromDht(prev_action_hash.clone().into()))?;
        check_prev_author(action, prev_action)?;
        check_prev_timestamp(action, prev_action)?;
        check_prev_seq(action, prev_action)?;
        check_agent_validation_pkg_predecessor(action, prev_action)?;
    }

    Ok(())
}

async fn store_entry(
    action: NewEntryActionRef<'_>,
    entry: &Entry,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let entry_type = action.entry_type();
    let entry_hash = action.entry_hash();

    // Checks
    check_entry_type(entry_type, entry)?;
    check_entry_hash(entry_hash, entry)?;
    check_entry_size(entry)?;

    // Additional checks if this is an Update
    if let NewEntryActionRef::Update(entry_update) = action {
        let original_action_address = &entry_update.original_action_address;
        let mut validation_dependencies = validation_dependencies.lock();
        let original_action = validation_dependencies
            .get(original_action_address)
            .and_then(|s| s.as_action())
            .ok_or_else(|| {
                ValidationOutcome::DepMissingFromDht(original_action_address.clone().into())
            })?;
        update_check(entry_update, original_action)?;
    }

    // Additional checks if this is a countersigned entry.
    if let Entry::CounterSign(session_data, _) = entry {
        check_countersigning_session_data(EntryHash::with_data_sync(entry), session_data, action)
            .await?;
    }
    Ok(())
}

fn register_updated_content(
    entry_update: &Update,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_action_address = &entry_update.original_action_address;

    let mut validation_dependencies = validation_dependencies.lock();
    let original_action = validation_dependencies
        .get(original_action_address)
        .and_then(|s| s.as_action())
        .ok_or_else(|| {
            ValidationOutcome::DepMissingFromDht(original_action_address.clone().into())
        })?;

    update_check(entry_update, original_action)
}

fn register_updated_record(
    record_update: &Update,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_action_address = &record_update.original_action_address;

    let mut validation_dependencies = validation_dependencies.lock();
    let original_action = validation_dependencies
        .get(original_action_address)
        .and_then(|s| s.as_action())
        .ok_or_else(|| {
            ValidationOutcome::DepMissingFromDht(original_action_address.clone().into())
        })?;

    update_check(record_update, original_action)
}

fn register_deleted_by(
    record_delete: &Delete,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_action_address = &record_delete.deletes_address;

    let mut validation_dependencies = validation_dependencies.lock();
    let action = validation_dependencies
        .get(removed_action_address)
        .and_then(|s| s.as_action())
        .ok_or_else(|| {
            ValidationOutcome::DepMissingFromDht(removed_action_address.clone().into())
        })?;

    check_new_entry_action(action)
}

fn register_deleted_entry_action(
    record_delete: &Delete,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_action_address = &record_delete.deletes_address;

    let mut validation_dependencies = validation_dependencies.lock();
    let action = validation_dependencies
        .get(removed_action_address)
        .and_then(|s| s.as_action())
        .ok_or_else(|| {
            ValidationOutcome::DepMissingFromDht(removed_action_address.clone().into())
        })?;

    check_new_entry_action(action)
}

fn register_add_link(link_add: &CreateLink) -> SysValidationResult<()> {
    check_tag_size(&link_add.tag)
}

fn register_delete_link(
    link_remove: &DeleteLink,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let link_add_address = &link_remove.link_add_address;

    // Just require that this link exists, don't need to check anything else about it here
    let mut validation_dependencies = validation_dependencies.lock();
    let add_link_action = validation_dependencies
        .get(link_add_address)
        .and_then(|s| s.as_action())
        .ok_or_else(|| ValidationOutcome::DepMissingFromDht(link_add_address.clone().into()))?;

    match add_link_action {
        Action::CreateLink(_) => Ok(()),
        _ => Err(ValidationOutcome::NotCreateLink(add_link_action.to_hash()).into()),
    }
}

fn update_check(entry_update: &Update, original_action: &Action) -> SysValidationResult<()> {
    check_new_entry_action(original_action)?;
    // This shouldn't fail due to the above `check_new_entry_action` check
    let original_action: NewEntryActionRef = original_action
        .try_into()
        .map_err(|_| ValidationOutcome::NotNewEntry(original_action.clone()))?;
    check_update_reference(entry_update, &original_action)?;
    Ok(())
}

pub struct SysValidationWorkspace {
    scratch: Option<SyncScratch>,
    authored_db: DbRead<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    dht_query_cache: Option<DhtDbQueryCache>,
    cache: DbWrite<DbKindCache>,
    pub(crate) dna_def: Arc<DnaDef>,
    sys_validation_retry_delay: Duration,
}

impl SysValidationWorkspace {
    pub fn new(
        authored_db: DbRead<DbKindAuthored>,
        dht_db: DbWrite<DbKindDht>,
        dht_query_cache: DhtDbQueryCache,
        cache: DbWrite<DbKindCache>,
        dna_def: Arc<DnaDef>,
        sys_validation_retry_delay: Duration,
    ) -> Self {
        Self {
            scratch: None,
            authored_db,
            dht_db,
            dht_query_cache: Some(dht_query_cache),
            cache,
            dna_def,
            sys_validation_retry_delay,
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
            .read_async(move |txn| {
                let mut stmt = txn.prepare(
                    "
                SELECT
                EXISTS (
                    SELECT
                    1
                    FROM Action
                    JOIN
                    DhtOp ON Action.hash = DhtOp.action_hash
                    WHERE
                    Action.author = :author
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

    pub async fn action_seq_is_empty(&self, action: &Action) -> SourceChainResult<bool> {
        let author = action.author().clone();
        let seq = action.action_seq();
        let hash = ActionHash::with_data_sync(action);
        let action_seq_is_not_empty = self
            .dht_db
            .read_async({
                let hash = hash.clone();
                move |txn| {
                    DatabaseResult::Ok(txn.query_row(
                        "
                SELECT EXISTS(
                    SELECT
                    1
                    FROM Action
                    WHERE
                    Action.author = :author
                    AND
                    Action.seq = :seq
                    AND
                    Action.hash != :hash
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
        let action_seq_is_not_empty = match &self.scratch {
            Some(scratch) => {
                scratch.apply(|scratch| {
                    scratch.actions().any(|shh| {
                        shh.action().action_seq() == seq && *shh.action_address() != hash
                    })
                })? || action_seq_is_not_empty
            }
            None => action_seq_is_not_empty,
        };
        Ok(!action_seq_is_not_empty)
    }

    /// Create a cascade with local data only
    pub fn local_cascade(&self) -> CascadeImpl {
        let cascade = CascadeImpl::empty()
            .with_dht(self.dht_db.clone().into())
            .with_cache(self.cache.clone());
        match &self.scratch {
            Some(scratch) => cascade
                .with_authored(self.authored_db.clone())
                .with_scratch(scratch.clone()),
            None => cascade,
        }
    }

    pub fn network_and_cache_cascade<Network: HolochainP2pDnaT>(
        &self,
        network: Network,
    ) -> CascadeImpl<Network> {
        CascadeImpl::empty().with_network(network, self.cache.clone())
    }

    /// Get a reference to the sys validation workspace's dna def.
    pub fn dna_def(&self) -> Arc<DnaDef> {
        self.dna_def.clone()
    }

    /// Get a reference to the sys validation workspace's dna def.
    pub fn dna_def_hashed(&self) -> DnaDefHashed {
        DnaDefHashed::from_content_sync((*self.dna_def).clone())
    }
}

fn put_validation_limbo(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    status: ValidationStage,
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
    Ok(())
}

#[derive(Debug, Clone)]
struct OutcomeSummary {
    accepted: usize,
    missing: usize,
    rejected: usize,
}

impl OutcomeSummary {
    fn new() -> Self {
        OutcomeSummary {
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
