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
//! The workflow operates on [`DhtOp`]s which can be either a [`ChainOp`] or a [`WarrantOp`].
//!
//! A [`ChainOp`] is a container for a [`Record`], sent to a specific type of authority.
//! Checks that you can rely on sys validation having performed are:
//! - For a [`ChainOp::StoreRecord`]
//!    - Check that the [`Action`] is either a [`Action::Dna`] at sequence number 0, or has a previous action with sequence number strictly greater than 0.
//!    - If the [`Entry`] is an [`Entry::CounterSign`], then the countersigning session data is mapped to a set of [`Action`]s and each of those actions must be found locally before this op can progress.
//!    - The [`Action`] must be either a [`Action::Create`] or an [`Action::Update`].
//!    - Run the [store entry checks](#store-entry-checks).
//! - For a [`ChainOp::StoreEntry`]
//!    - If the [`Entry`] is an [`Entry::CounterSign`], then the countersigning session data is mapped to a set of [`Action`]s and each of those actions must be found locally before this op is accepted.
//!    - Check that the [`Action`] is either a [`Action::Dna`] at sequence number 0, or has a previous action with sequence number strictly greater than 0.
//!    - Run the [store entry checks](#store-entry-checks).
//! - For a [`ChainOp::RegisterAgentActivity`]
//!    - Check that the [`Action`] is either a [`Action::Dna`] at sequence number 0, or has a previous action with sequence number strictly greater than 0.
//!    - If the [`Action`] is a [`Action::Dna`], then verify the contained DNA hash matches the DNA hash that sys validation is being run for.
//!    - Check that the previous action is never a [`Action::CloseChain`], since this is always required to be the last action in a chain.
//!    - Run the [store record checks](#store-record-checks).
//! - For a [`ChainOp::RegisterUpdatedContent`]
//!    - The [`Update::original_action_address`] reference to the [`Action`] being updated must point to an [`Action`] that can be found locally. Once the [`Action`] address has been resolved, the [`Update::original_entry_address`] is checked against the entry address that the referenced [`Action`] specified.
//!    - If there is an [`Entry`], then the [store entry checks](#store-entry-checks) are run.
//! - For a [`ChainOp::RegisterUpdatedRecord`]
//!    - The [`Update::original_action_address`] reference to the [`Action`] being updated must point to an [`Action`] that can be found locally. Once the [`Action`] address has been resolved, the [`Update::original_entry_address`] is checked against the entry address that the referenced [`Action`] specified.
//!    - If there is an [`Entry`], then the [store entry checks](#store-entry-checks) are run.
//! - For a [`ChainOp::RegisterDeletedBy`]
//!    - The [`Delete::deletes_address`] reference to the [`Action`] being deleted must point to an [`Action`] that can be found locally. The action being deleted must be a [`Action::Create`] or [`Action::Update`].
//! - For a [`ChainOp::RegisterDeletedEntryAction`]
//!    - The [`Delete::deletes_address`] reference to the [`Action`] being deleted must point to an [`Action`] that can be found locally. The action being deleted must be a [`Action::Create`] or [`Action::Update`].
//! - For a [`ChainOp::RegisterAddLink`]
//!   - The size of the [`CreateLink::tag`] must be less than or equal to the maximum size that is accepted for this link tag. This is specified in the constant [`MAX_TAG_SIZE`].
//! - For a [`ChainOp::RegisterRemoveLink`]
//!   - The [`DeleteLink::link_add_address`] reference to the [`Action`] of the link being deleted must point to an [`Action`] that can be found locally. That action being deleted must also
//!     be a [`Action::CreateLink`].
//!
//! A [`WarrantOp`] is produced as a proof that an agent broke the rules of the DHT. That may be the
//! rules of this workflow or the rules set by the application and checked in the app validation
//! workflow.
//! Warrant validation is performed entirely by sys validation, and the steps performed are:
//! - For a [`WarrantOpType::ChainIntegrityWarrant`]:
//!   - The warranted [`Action`] must be found locally. If it is not found, then it will be fetched.
//!   - The warranted [`Action`] will be completely validated, including app validation.
//!   - If the [`Action`] is rejected, then the warrant is accepted, and the author of the warrant
//!     will be blocked.
//!   - If the [`Action`] is accepted, then the warrant is rejected. The author of the warrant will
//!     then be blocked.
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
//! - Runs the store entry checks if there is an entry present on the record.
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
//!     - For any chain ops that passed validation, they will be marked as ready for app validation in the database.
//!     - Any chain ops that were rejected will be marked rejected in the database.
//!     - Any warrant ops that were accepted will be marked as valid in the database.
//!     - Any warrant ops that were rejected will be marked as rejected in the database.
//! - If any ops passed validation, then app validation will be triggered.
//! - For action dependencies not found locally, the workflow will then attempt to fetch them from the network.
//! - If any actions that were missing are found on the network, then sys validation is re-triggered to see if the newly fetched actions allow any outstanding ops to pass validation.
//! - If fewer actions were fetched from the network than there were actions missing, then the workflow will sleep for a short time before re-triggering itself.
//! - Once all ops have an outcome, the workflow is complete and will wait to be triggered again by new incoming ops.
//!

use self::validation_deps::SysValDeps;
use self::validation_deps::ValidationDependencies;
use self::validation_deps::ValidationDependencyState;
use crate::conductor::Conductor;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::sys_validate::*;
use crate::core::validation::*;
use crate::core::workflow::error::WorkflowResult;
use crate::core::workflow::sys_validation_workflow::validation_deps::{
    ValidationDependencyType, ValidationDependencyValue, WarrantedDep,
};
use crate::core::workflow::WorkflowError;
use futures::FutureExt;
use futures::StreamExt;
use holo_hash::DhtOpHash;
use holochain_cascade::Cascade;
use holochain_cascade::CascadeImpl;
use holochain_keystore::MetaLairClient;
use holochain_p2p::DynHolochainP2pDna;
use holochain_sqlite::prelude::*;
use holochain_sqlite::sql::sql_cell::ACTION_HASH_BY_PREV;
#[cfg(feature = "unstable-warrants")]
use holochain_state::integrate::insert_locally_validated_op;
use holochain_state::prelude::*;
use rusqlite::Transaction;
use std::convert::TryInto;
use std::sync::Arc;
use std::time::Duration;
use types::Outcome;

pub mod types;

pub mod validation_deps;
pub mod validation_query;

#[cfg(test)]
mod chain_test;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod unit_tests;
#[cfg(test)]
mod validate_op_tests;

/// The sys validation workflow. It is described in the module level documentation.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub async fn sys_validation_workflow(
    workspace: Arc<SysValidationWorkspace>,
    current_validation_dependencies: SysValDeps,
    trigger_app_validation: TriggerSender,
    trigger_integration: TriggerSender,
    trigger_publish: TriggerSender,
    trigger_self: TriggerSender,
    network: DynHolochainP2pDna,
    keystore: MetaLairClient,
    representative_agent: AgentPubKey,
) -> WorkflowResult<WorkComplete> {
    // Run the actual sys validation using data we have locally
    let outcome_summary = sys_validation_workflow_inner(
        workspace.clone(),
        current_validation_dependencies.clone(),
        network.clone(),
        keystore,
        representative_agent,
    )
    .await?;

    // trigger app validation to process any ops that have been processed so far
    if outcome_summary.accepted > 0 {
        tracing::debug!("Sys validation accepted {} ops", outcome_summary.accepted);

        trigger_app_validation.trigger(&"sys_validation_workflow");
    }

    if outcome_summary.warranted > 0 {
        tracing::debug!(
            "Sys validation created {} warrants",
            outcome_summary.warranted
        );

        trigger_publish.trigger(&"sys_validation_workflow");
        trigger_integration.trigger(&"sys_validation_workflow");
    } else if outcome_summary.warrants_rejected > 0 {
        tracing::debug!(
            "Sys validation rejected {} warrants",
            outcome_summary.warrants_rejected
        );

        trigger_integration.trigger(&"sys_validation_workflow");
    }

    // Now go to the network to try to fetch missing dependencies
    let num_fetched =
        fetch_missing_dependencies(&workspace, network, current_validation_dependencies).await;

    if outcome_summary.missing > 0 {
        tracing::debug!(
            "Fetched {}/{} missing dependencies from the network",
            num_fetched,
            outcome_summary.missing
        );
    }

    if num_fetched > 0 || outcome_summary.warrant_deps_copied > 0 {
        // - If we fetched anything, then we can re-run sys validation
        // - If any warrant dependencies were copied, the workflow could attempt validation for those
        //   right away, so trigger it again.
        trigger_self.trigger(&"sys_validation_workflow");
    }

    if num_fetched < outcome_summary.missing {
        tracing::info!(
            "Sys validation sleeping for {:?}, with {num_fetched} fetched of {} missing dependencies",
            workspace.sys_validation_retry_delay,
            outcome_summary.missing
        );
        Ok(WorkComplete::Incomplete(Some(
            workspace.sys_validation_retry_delay,
        )))
    } else {
        Ok(WorkComplete::Complete)
    }
}

type ForkedPair = ((ActionHash, Signature), (ActionHash, Signature));

async fn sys_validation_workflow_inner(
    workspace: Arc<SysValidationWorkspace>,
    current_validation_dependencies: SysValDeps,
    _network: DynHolochainP2pDna,
    _keystore: MetaLairClient,
    _representative_agent: AgentPubKey,
) -> WorkflowResult<OutcomeSummary> {
    let db = workspace.dht_db.clone();
    let sorted_ops = validation_query::get_ops_to_sys_validate(&db).await?;

    // Forget what dependencies are currently in use
    current_validation_dependencies
        .lock()
        .expect("poisoned")
        .clear_retained_deps();

    if sorted_ops.is_empty() {
        tracing::trace!(
            "Skipping sys_validation_workflow because there are no ops to be validated"
        );

        // If there's nothing to validate then we can clear the dependencies and save some memory.
        current_validation_dependencies
            .lock()
            .expect("poisoned")
            .purge_held_deps();

        return Ok(OutcomeSummary::new());
    }

    let num_ops_to_validate = sorted_ops.len();
    tracing::debug!("Sys validating {} ops", num_ops_to_validate);

    let cascade = Arc::new(workspace.local_cascade());

    retrieve_previous_actions_for_ops(
        current_validation_dependencies.clone(),
        cascade.clone(),
        sorted_ops.clone().into_iter(),
    )
    .await;

    // Now drop all the dependencies that we didn't just try to access while searching the current set of ops to validate.
    current_validation_dependencies
        .lock()
        .expect("poisoned")
        .purge_held_deps();

    let mut warrant_deps_copied = 0;
    move_and_check_warrant_deps(
        &workspace,
        &current_validation_dependencies,
        &mut warrant_deps_copied,
    )
    .await;

    let mut validation_outcomes = Vec::with_capacity(sorted_ops.len());
    for hashed_op in sorted_ops {
        // Note that this is async only because of the signature checks done during countersigning.
        // In most cases this will be a fast synchronous call.
        let r = validate_op(
            hashed_op.as_content(),
            &workspace.dna_hash,
            current_validation_dependencies.clone(),
        )
        .await;

        match r {
            Ok(outcome) => validation_outcomes.push((hashed_op, outcome)),
            Err(e) => {
                tracing::error!(error = ?e, "Error validating op");
            }
        }
    }

    // Allow unused mutable, because it's mutated when feature unstable-warrants is enabled.
    #[allow(unused_mut)]
    let (mut summary, _invalid_ops, _forked_pairs) = workspace
        .dht_db
        .write_async(move |txn| {
            let mut summary = OutcomeSummary {
                warrant_deps_copied,
                ..Default::default()
            };
            let mut invalid_ops = vec![];
            let mut forked_pairs: Vec<(AgentPubKey, ForkedPair)> = vec![];

            for (hashed_op, outcome) in validation_outcomes {
                let (op, op_hash) = hashed_op.into_inner();
                let op_type = op.get_type();

                #[cfg(feature = "unstable-warrants")]
                if let DhtOp::ChainOp(chain_op) = &op {
                    // Author a ChainFork warrant if fork is detected
                    let action = chain_op.action();
                    if let Some(forked_action) = detect_fork(txn, &action)? {
                        let signature = chain_op.signature().clone();
                        let action_hash = action.to_hash();
                        forked_pairs.push((
                            action.author().clone(),
                            ((action_hash, signature), forked_action),
                        ));
                    }
                }

                match outcome {
                    Outcome::Accepted => {
                        summary.accepted += 1;
                        match op_type {
                            DhtOpType::Chain(_) => {
                                put_validation_limbo(txn, &op_hash, ValidationStage::SysValidated)?
                            }
                            DhtOpType::Warrant(_) =>
                            {
                                #[cfg(feature = "unstable-warrants")]
                                put_integration_limbo(txn, &op_hash, ValidationStatus::Valid)?
                            }
                        };
                    }
                    Outcome::MissingDhtDep => {
                        summary.missing += 1;
                        let status = ValidationStage::AwaitingSysDeps;
                        put_validation_limbo(txn, &op_hash, status)?;
                    }
                    Outcome::Rejected(_) => {
                        invalid_ops.push((op_hash.clone(), op.clone()));

                        match op {
                            DhtOp::ChainOp(_) => {
                                summary.rejected += 1;
                            }
                            DhtOp::WarrantOp(_) => {
                                summary.warrants_rejected += 1;
                            }
                        }

                        put_integration_limbo(txn, &op_hash, ValidationStatus::Rejected)?;
                    }
                }
            }
            WorkflowResult::Ok((summary, invalid_ops, forked_pairs))
        })
        .await?;

    #[cfg(feature = "unstable-warrants")]
    {
        let mut warrants = vec![];
        for (op_hash, chain_op) in _invalid_ops
            .into_iter()
            .filter_map(|(h, op)| op.as_chain_op().map(|op| (h, op.clone())))
        {
            match holochain_state::warrant::is_action_warranted_as_invalid(
                &workspace.dht_db,
                chain_op.action().to_hash(),
                chain_op.author().clone(),
            )
            .await
            {
                Ok(true) => {
                    tracing::trace!(
                        "Op {} is already warranted, not issuing a new warrant",
                        op_hash
                    );
                    continue;
                }
                Ok(false) => {
                    // Not warranted yet, should issue a warrant.
                }
                Err(e) => {
                    tracing::error!(error = ?e, "Error checking if op is warranted");
                    continue;
                }
            }

            let warrant_op =
                make_invalid_chain_warrant_op(&_keystore, _representative_agent.clone(), &chain_op)
                    .await?;
            warrants.push(warrant_op);
        }

        for (author, pair) in _forked_pairs {
            let warrant_op =
                make_fork_warrant_op_inner(&_keystore, _representative_agent.clone(), author, pair)
                    .await?;
            warrants.push(warrant_op);
        }

        let authored_warrants = warrants.clone();
        let warranted = workspace
            .authored_db
            .write_async(move |txn| {
                let mut warranted = 0;
                for warrant_op in authored_warrants {
                    insert_op_authored(txn, &warrant_op)?;
                    warranted += 1;
                }
                StateMutationResult::Ok(warranted)
            })
            .await?;
        // "self-publish" warrants, i.e. insert them into the DHT DB.
        // This works around the problem that the commonly used function [`holochain_state::integrate::authored_ops_to_dht_db`]
        // joins on the Action table to filter out private entries.
        workspace
            .dht_db
            .write_async(move |txn| {
                for warrant_op in warrants {
                    insert_locally_validated_op(txn, warrant_op)?;
                }
                StateMutationResult::Ok(())
            })
            .await?;

        summary.warranted = warranted;
    }

    tracing::debug!(
        ?summary,
        ?num_ops_to_validate,
        "Finished sys validation workflow"
    );

    Ok(summary)
}

async fn fetch_missing_dependencies(
    workspace: &SysValidationWorkspace,
    network: DynHolochainP2pDna,
    current_validation_dependencies: SysValDeps,
) -> usize {
    let network_cascade = Arc::new(workspace.network_and_cache_cascade(network));
    let missing_dependencies = current_validation_dependencies
        .lock()
        .expect("poisoned")
        .get_missing_dependencies();

    futures::stream::iter(missing_dependencies.into_iter().map(|(hash, dep_type)| {
        let network_cascade = network_cascade.clone();
        let current_validation_dependencies = current_validation_dependencies.clone();
        async move {
            match dep_type {
                ValidationDependencyType::Action => {
                    match network_cascade
                        .retrieve_action(hash, Default::default())
                        .await
                    {
                        Ok(Some((action, source))) => {
                            let mut deps =
                                current_validation_dependencies.lock().expect("poisoned");

                            if deps.insert_action(action, source) {
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
                        Err(err) => {
                            tracing::error!(?err, "Error fetching missing dependency");
                            0
                        }
                    }
                }
                ValidationDependencyType::Warranted(chain_op_type) => {
                    match network_cascade
                        .retrieve_public_record(hash.clone().into(), Default::default())
                        .await
                    {
                        Ok(Some((record, source))) => {
                            let mut deps =
                                current_validation_dependencies.lock().expect("poisoned");

                            if deps.insert_pending_validation_warranted(
                                record.signed_action,
                                chain_op_type,
                                source,
                            ) {
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
                        Err(err) => {
                            tracing::error!(?err, "Error fetching missing dependency");
                            0
                        }
                    }
                }
            }
        }
        .boxed()
    }))
    .buffer_unordered(10)
    .collect::<Vec<usize>>()
    .await
    .into_iter()
    .sum()
}

// - Move any cached warranted ops because they need to be validated now.
// - Check the validation status of any warranted ops that are in the DHT database.
// - Any ops that have a validation status can be used to mark warrants as ready to validate.
async fn move_and_check_warrant_deps(
    workspace: &Arc<SysValidationWorkspace>,
    current_validation_dependencies: &SysValDeps,
    warrant_deps_copied: &mut usize,
) {
    let warrant_deps = current_validation_dependencies
        .lock()
        .expect("poisoned")
        .get_pending_warrant_dependencies();
    for (action_hash, op_type) in warrant_deps {
        match copy_cached_op_to_dht(
            workspace.dht_db.clone(),
            workspace.cache.clone().into(),
            action_hash.clone(),
            op_type,
        )
        .await
        {
            Ok(copied) if copied => {
                *warrant_deps_copied += 1;
            }
            Ok(_) => {
                // It's in the DHT already.
            }
            Err(StateMutationError::OpNotFoundInCache) => {
                tracing::debug!("Warranted op {} not found in cache", action_hash);
                continue;
            }
            Err(e) => {
                tracing::error!(error = ?e, "Error copying warranted op to DHT");
                continue;
            }
        }

        match get_dht_op_validation_state(
            &workspace.dht_db.clone().into(),
            action_hash.clone(),
            op_type,
        )
        .await
        {
            Ok(Some((_, Some(validation_status)))) => {
                // The dependency has been validated. The warrant dependency is ready to be used to
                // check the warrant.
                current_validation_dependencies
                    .lock()
                    .expect("poisoned")
                    .update_warrant_dep_validated(&action_hash, validation_status);
            }
            Ok(Some((stage, None))) => {
                // The dependency is present but not yet validated.
                tracing::debug!(?action_hash, ?stage, "Warranted op not yet validated");
            }
            Ok(None) => {
                // The dependency is not yet in the DHT database.
                tracing::debug!(?action_hash, "Warranted op not yet in DHT database");
            }
            Err(e) => {
                tracing::error!(error = ?e, "Error checking warranted op validation state");
            }
        }
    }
}

async fn retrieve_dependencies(
    current_validation_dependencies: SysValDeps,
    cascade: Arc<impl Cascade + Send + Sync>,
    dependencies: impl Iterator<Item = (ActionHash, ValidationDependencyType)>,
) {
    let dependency_fetches = dependencies
        .filter(|(hash, _)| {
            !current_validation_dependencies
                .lock()
                .expect("poisoned")
                .has(hash)
        })
        .map(|(hash, dep_type)| {
            // For each previous action that will be needed for validation, map the action to a fetch Action for its hash
            let cascade = cascade.clone();
            async move {
                match dep_type {
                    ValidationDependencyType::Action => {
                        let fetched = cascade.retrieve_action(hash.clone(), Default::default()).await;
                        tracing::trace!(hash = ?hash, fetched = ?fetched, "Fetched action for validation");
                        (hash, dep_type, fetched.map(|ok| ok.map(|(a, s)| (ValidationDependencyValue::Action(a), s))))
                    }
                    ValidationDependencyType::Warranted(chain_op_type) => {
                        let fetched = match chain_op_type {
                            // These chain op types require the entry to be present.
                            ChainOpType::StoreRecord |
                            ChainOpType::StoreEntry |
                            ChainOpType::RegisterUpdatedContent |
                            ChainOpType::RegisterUpdatedRecord => {
                                cascade.retrieve_public_record(hash.clone().into(), Default::default()).await.map(|ok|ok.map(|(a, s)|(a.signed_action, s)))
                            }
                            // Other top types can be constructed without an entry.
                            _ => {
                                cascade.retrieve_action(hash.clone(), Default::default()).await
                            }
                        };
                        tracing::trace!(hash = ?hash, fetched = ?fetched, "Fetched warranted record for validation");
                        (hash, dep_type, fetched.map(|ok| ok.map(|(a, s)| (ValidationDependencyValue::Warranted(WarrantedDep::Pending(a, chain_op_type)), s))))
                    }
                }
            }
                .boxed()
        });

    let new_deps: ValidationDependencies = ValidationDependencies::new_from_iter(futures::future::join_all(dependency_fetches)
        .await
        .into_iter()
        .filter_map(|r| {
            // Filter out errors, preparing the rest to be put into a HashMap for easy access.
            match r {
                (hash, _, Ok(Some((value, source)))) => {
                    Some((hash, ValidationDependencyState::new_present(value, source)))
                }
                (hash, dep_type, Ok(None)) => {
                    Some((hash, ValidationDependencyState::new_pending(dep_type)))
                }
                (hash, dep_type, Err(e)) => {
                    tracing::error!(error = ?e, action_hash = ?hash, ?dep_type, "Error retrieving prev action");
                    None
                }
            }
        }));

    current_validation_dependencies
        .lock()
        .expect("poisoned")
        .merge(new_deps);
}

fn get_dependency_hashes_from_actions(
    actions: impl Iterator<Item = Action>,
) -> Vec<(ActionHash, ValidationDependencyType)> {
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
            .filter_map(|opt| opt.map(|a| (a, ValidationDependencyType::Action)))
        })
        .collect()
}

/// Examine the list of provided actions and create a list of actions which are sys validation
/// dependencies for those actions. The actions are merged into `current_validation_dependencies`.
async fn fetch_previous_actions(
    current_validation_dependencies: SysValDeps,
    cascade: Arc<impl Cascade + Send + Sync>,
    actions: impl Iterator<Item = Action>,
) {
    retrieve_dependencies(
        current_validation_dependencies,
        cascade,
        get_dependency_hashes_from_actions(actions).into_iter(),
    )
    .await;
}

fn get_dependencies_from_ops(
    ops: impl Iterator<Item = DhtOpHashed>,
) -> Vec<(ActionHash, ValidationDependencyType)> {
    ops.into_iter()
        .filter_map(|op| {
            // For each previous action that will be needed for validation, map the action to a fetch Record for its hash
            match &op.content {
                DhtOp::ChainOp(op) => {
                    let chain_op_dependencies = match &**op {
                        ChainOp::StoreRecord(_, action, entry) => {
                            let mut actions = match entry {
                                // TODO we should be doing something similar to this in app validation!
                                RecordEntry::Present(
                                    entry @ Entry::CounterSign(session_data, _),
                                ) => {
                                    // Discard errors here because we'll check later whether the input is valid. If it's not then it
                                    // won't matter that we've skipped fetching deps for it
                                    if let Ok(entry_rate_weight) =
                                        action_to_entry_rate_weight(action)
                                    {
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
                        ChainOp::StoreEntry(_, action, entry) => {
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
                        ChainOp::RegisterAgentActivity(_, action) => action
                            .prev_action()
                            .map(|action| vec![action.as_hash().clone()]),
                        ChainOp::RegisterUpdatedContent(_, action, _) => {
                            Some(vec![action.original_action_address.clone()])
                        }
                        ChainOp::RegisterUpdatedRecord(_, action, _) => {
                            Some(vec![action.original_action_address.clone()])
                        }
                        ChainOp::RegisterDeletedBy(_, action) => {
                            Some(vec![action.deletes_address.clone()])
                        }
                        ChainOp::RegisterDeletedEntryAction(_, action) => {
                            Some(vec![action.deletes_address.clone()])
                        }
                        ChainOp::RegisterRemoveLink(_, action) => {
                            Some(vec![action.link_add_address.clone()])
                        }
                        _ => None,
                    };

                    chain_op_dependencies.map(|deps| {
                        deps.into_iter()
                            .map(|hash| (hash, ValidationDependencyType::Action))
                            .collect::<Vec<_>>()
                    })
                }
                DhtOp::WarrantOp(op) => match &op.proof {
                    WarrantProof::ChainIntegrity(warrant) => match warrant {
                        ChainIntegrityWarrant::InvalidChainOp {
                            action: (action_hash, _),
                            chain_op_type,
                            ..
                        } => Some(vec![(
                            action_hash.clone(),
                            ValidationDependencyType::Warranted(*chain_op_type),
                        )]),
                        ChainIntegrityWarrant::ChainFork {
                            action_pair: ((a1, _), (a2, _)),
                            ..
                        } => Some(vec![
                            (a1.clone(), ValidationDependencyType::Action),
                            (a2.clone(), ValidationDependencyType::Action),
                        ]),
                    },
                },
            }
        })
        .flatten()
        .collect()
}

/// Examine the list of provided ops and create a list of actions which are sys validation
/// dependencies for those ops. The actions are merged into `current_validation_dependencies`.
async fn retrieve_previous_actions_for_ops(
    current_validation_dependencies: SysValDeps,
    cascade: Arc<impl Cascade + Send + Sync>,
    ops: impl Iterator<Item = DhtOpHashed>,
) {
    retrieve_dependencies(
        current_validation_dependencies,
        cascade,
        get_dependencies_from_ops(ops).into_iter(),
    )
    .await;
}

/// Validate a single DhtOp, using the supplied Cascade to draw dependencies from
pub(crate) async fn validate_op(
    op: &DhtOp,
    dna_hash: &DnaHash,
    validation_dependencies: SysValDeps,
) -> WorkflowResult<Outcome> {
    let result = match op {
        DhtOp::ChainOp(op) => validate_chain_op(op, dna_hash, validation_dependencies).await,
        DhtOp::WarrantOp(op) => validate_warrant_op(op, validation_dependencies).await,
    };
    match result {
        Ok(_) => Ok(Outcome::Accepted),
        // Handle the errors that result in pending or awaiting deps
        Err(SysValidationError::ValidationOutcome(e)) => {
            if e.is_indeterminate() {
                // This is expected if the dependency isn't held locally and needs to be fetched from the network
                // so downgrade the logging to trace.
                tracing::debug!(
                    msg = "DhtOp has a missing dependency",
                    ?op,
                    error = ?e,
                    error_msg = %e
                );
                Ok(Outcome::MissingDhtDep)
            } else {
                tracing::warn!(msg = "DhtOp was rejected during system validation.", ?op, error = ?e, error_msg = %e);
                Ok(Outcome::Rejected(e.to_string()))
            }
        }
        Err(e) => Err(e.into()),
    }
}

fn action_to_entry_rate_weight(action: &Action) -> SysValidationResult<EntryRateWeight> {
    action
        .entry_rate_data()
        .ok_or_else(|| SysValidationError::NonEntryAction(Box::new(action.clone())))
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

async fn validate_chain_op(
    op: &ChainOp,
    dna_hash: &DnaHash,
    validation_dependencies: SysValDeps,
) -> SysValidationResult<()> {
    check_entry_visibility(op)?;
    match op {
        ChainOp::StoreRecord(_, action, entry) => {
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
                        let validation_dependencies =
                            validation_dependencies.lock().expect("poisoned");
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
                        .map_err(|_| ValidationOutcome::NotNewEntry(Box::new(action.clone())))?,
                    entry,
                    validation_dependencies,
                )
                .await?;
            }
            Ok(())
        }
        ChainOp::StoreEntry(_, action, entry) => {
            // Check and hold for all other actions on countersigned entry.
            if let Entry::CounterSign(session_data, _) = entry {
                for action_hash in make_action_set_for_session_data(
                    new_entry_action_to_entry_rate_weight(action),
                    entry,
                    session_data,
                )? {
                    // Just require that we are holding all the other actions
                    let validation_dependencies = validation_dependencies.lock().expect("poisoned");
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
        ChainOp::RegisterAgentActivity(_, action) => {
            register_agent_activity(action, validation_dependencies.clone(), dna_hash)?;
            store_record(action, validation_dependencies)
        }
        ChainOp::RegisterUpdatedContent(_, action, entry) => {
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
        ChainOp::RegisterUpdatedRecord(_, action, entry) => {
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
        ChainOp::RegisterDeletedBy(_, action) => {
            register_deleted_by(action, validation_dependencies)
        }
        ChainOp::RegisterDeletedEntryAction(_, action) => {
            register_deleted_entry_action(action, validation_dependencies)
        }
        ChainOp::RegisterAddLink(_, action) => register_add_link(action),
        ChainOp::RegisterRemoveLink(_, action) => {
            register_delete_link(action, validation_dependencies)
        }
    }
}

async fn validate_warrant_op(
    op: &WarrantOp,
    validation_dependencies: SysValDeps,
) -> SysValidationResult<()> {
    match &op.proof {
        WarrantProof::ChainIntegrity(warrant) => match warrant {
            ChainIntegrityWarrant::InvalidChainOp {
                action: (action_hash, action_sig),
                action_author,
                ..
            } => {
                let (action, validation_status) = {
                    let deps = validation_dependencies.lock().expect("poisoned");
                    let (action, validation_status) = deps
                        .get(action_hash)
                        .and_then(|s| s.as_action_and_validation_status())
                        .ok_or_else(|| {
                            ValidationOutcome::DepMissingFromDht(action_hash.clone().into())
                        })?;

                    if action.author() != action_author {
                        return Err(ValidationOutcome::InvalidWarrant(
                            Box::new(op.warrant().clone()),
                            "action author mismatch".into(),
                        )
                        .into());
                    }
                    (action.clone(), validation_status)
                };
                verify_action_signature(action_sig, &action).await?;

                match validation_status {
                    ValidationStatus::Valid => {
                        return Err(SysValidationError::ValidationOutcome(
                            ValidationOutcome::InvalidWarrant(
                                Box::new(op.warrant().clone()),
                                "Warranted op is valid".into(),
                            ),
                        ));
                    }
                    ValidationStatus::Abandoned => {
                        // TODO This isn't really the right way to handle this. If the op is abandoned
                        //       then this warrant should probably also be abandoned. But there is
                        //       no implementation of abandon yet.
                        return Err(SysValidationError::WorkflowError(Box::new(
                            WorkflowError::Other(Box::new(std::io::Error::other(
                                "Warranted op is abandoned",
                            ))),
                        )));
                    }
                    ValidationStatus::Rejected => {
                        // This is good, the warrant is valid because we also rejected the op
                    }
                }

                Ok(())
            }
            ChainIntegrityWarrant::ChainFork {
                action_pair: ((a1, a1_sig), (a2, a2_sig)),
                chain_author,
                ..
            } => {
                let (action1, action2) = {
                    let deps = validation_dependencies.lock().expect("poisoned");
                    let action1 = deps
                        .get(a1)
                        .and_then(|s| s.as_action())
                        .ok_or_else(|| ValidationOutcome::DepMissingFromDht(a1.clone().into()))?;
                    let action2 = deps
                        .get(a2)
                        .and_then(|s| s.as_action())
                        .ok_or_else(|| ValidationOutcome::DepMissingFromDht(a2.clone().into()))?;

                    // chain_author basis must match the author of the action
                    if action1.author() != chain_author {
                        return Err(ValidationOutcome::InvalidWarrant(
                            Box::new(op.warrant().clone()),
                            "chain author mismatch".into(),
                        )
                        .into());
                    }

                    // Both actions must be by same author
                    if action1.author() != action2.author() {
                        return Err(ValidationOutcome::InvalidWarrant(
                            Box::new(op.warrant().clone()),
                            "action pair author mismatch".into(),
                        )
                        .into());
                    }

                    // A fork is evidenced by two actions with a common predecessor.
                    // NOTE: we could also check sequence numbers, but then we'd have to traverse
                    // both forks backwards until reaching a common ancestor to protect against an
                    // attack where someone authors a warrant using two actions from two different DNAs.
                    // Using seq numbers makes it easier to detect and prove a fork, but using prev_action
                    // prevents the attack.
                    if action1.prev_action() != action2.prev_action() {
                        return Err(ValidationOutcome::InvalidWarrant(
                            Box::new(op.warrant().clone()),
                            "action pair seq mismatch".into(),
                        )
                        .into());
                    }

                    (action1.clone(), action2.clone())
                };

                verify_action_signature(a1_sig, &action1).await?;
                verify_action_signature(a2_sig, &action2).await?;

                Ok(())
            }
        },
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
            tracing::error!(
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
    counterfeit_check_action(signature, action).await?;

    async fn validate(
        action: &Action,
        maybe_entry: Option<&Entry>,
        cascade: Arc<impl Cascade + Send + Sync>,
    ) -> SysValidationResult<()> {
        let validation_dependencies = SysValDeps::default();
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
                    .map_err(|_| ValidationOutcome::NotNewEntry(Box::new(action.clone())))?,
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

/// Check if the chain op has valid signature and author.
/// Ops that fail this check should be dropped.
pub async fn counterfeit_check_action(
    signature: &Signature,
    action: &Action,
) -> SysValidationResult<()> {
    verify_action_signature(signature, action).await?;
    Ok(())
}

/// Check if the warrant op has valid signature and author.
pub async fn counterfeit_check_warrant(warrant_op: &WarrantOp) -> SysValidationResult<()> {
    verify_warrant_signature(warrant_op).await?;
    Ok(())
}

fn register_agent_activity(
    action: &Action,
    validation_dependencies: SysValDeps,
    dna_hash: &DnaHash,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let prev_action_hash = action.prev_action();

    // Checks
    check_prev_action(action)?;
    check_valid_if_dna(action, dna_hash)?;
    if let Some(prev_action_hash) = prev_action_hash {
        // Just make sure we have the dependency and if not then don't mark this action as valid yet
        let validation_dependencies = validation_dependencies.lock().expect("poisoned");
        let prev_action = validation_dependencies
            .get(prev_action_hash)
            .and_then(|s| s.as_action())
            .ok_or_else(|| ValidationOutcome::DepMissingFromDht(prev_action_hash.clone().into()))?;

        match prev_action {
            Action::CloseChain(_) => Err(ValidationOutcome::PrevActionError(
                (PrevActionErrorKind::ActionAfterChainClose, action.clone()).into(),
            )
            .into()),
            _ => Ok(()),
        }
    } else {
        Ok(())
    }
}

fn store_record(action: &Action, validation_dependencies: SysValDeps) -> SysValidationResult<()> {
    // Get data ready to validate
    let prev_action_hash = action.prev_action();

    // Checks
    check_prev_action(action)?;
    if let Some(prev_action_hash) = prev_action_hash {
        let validation_dependencies = validation_dependencies.lock().expect("poisoned");
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
    validation_dependencies: SysValDeps,
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
        let validation_dependencies = validation_dependencies.lock().expect("poisoned");
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
    validation_dependencies: SysValDeps,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_action_address = &entry_update.original_action_address;

    let validation_dependencies = validation_dependencies.lock().expect("poisoned");
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
    validation_dependencies: SysValDeps,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_action_address = &record_update.original_action_address;

    let validation_dependencies = validation_dependencies.lock().expect("poisoned");
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
    validation_dependencies: SysValDeps,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_action_address = &record_delete.deletes_address;

    let validation_dependencies = validation_dependencies.lock().expect("poisoned");
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
    validation_dependencies: SysValDeps,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_action_address = &record_delete.deletes_address;

    let validation_dependencies = validation_dependencies.lock().expect("poisoned");
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
    validation_dependencies: SysValDeps,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let link_add_address = &link_remove.link_add_address;

    // Just require that this link exists, don't need to check anything else about it here
    let validation_dependencies = validation_dependencies.lock().expect("poisoned");
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
        .map_err(|_| ValidationOutcome::NotNewEntry(Box::new(original_action.clone())))?;
    check_update_reference(entry_update, &original_action)?;
    Ok(())
}

pub struct SysValidationWorkspace {
    scratch: Option<SyncScratch>,
    // Authored DB is writeable because warrants may be written.
    authored_db: DbWrite<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
    cache: DbWrite<DbKindCache>,
    dna_hash: DnaHash,
    sys_validation_retry_delay: Duration,
}

impl SysValidationWorkspace {
    pub fn new(
        authored_db: DbWrite<DbKindAuthored>,
        dht_db: DbWrite<DbKindDht>,
        cache: DbWrite<DbKindCache>,
        dna_hash: DnaHash,
        sys_validation_retry_delay: Duration,
    ) -> Self {
        Self {
            scratch: None,
            authored_db,
            dht_db,
            cache,
            dna_hash,
            sys_validation_retry_delay,
        }
    }

    /// Create a cascade with local data only
    pub fn local_cascade(&self) -> CascadeImpl {
        let cascade = CascadeImpl::empty()
            .with_dht(self.dht_db.clone().into())
            .with_cache(self.cache.clone());
        match &self.scratch {
            Some(scratch) => cascade
                .with_authored(self.authored_db.clone().into())
                .with_scratch(scratch.clone()),
            None => cascade,
        }
    }

    pub fn network_and_cache_cascade(&self, network: DynHolochainP2pDna) -> CascadeImpl {
        CascadeImpl::empty().with_network(network, self.cache.clone())
    }
}

fn put_validation_limbo(
    txn: &mut Transaction<'_>,
    hash: &DhtOpHash,
    stage: ValidationStage,
) -> WorkflowResult<()> {
    set_validation_stage(txn, hash, stage)?;
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

/// Gets an arbitrary agent with a cell running the given DNA, needed for processes
/// which require an agent signature but happen at the DNA level, i.e. not bound to any
/// particular cell.
pub fn get_representative_agent(conductor: &Conductor, dna_hash: &DnaHash) -> Option<AgentPubKey> {
    conductor
        .running_cell_ids()
        .into_iter()
        .find(|id| id.dna_hash() == dna_hash)
        .map(|id| id.agent_pubkey().clone())
}

pub async fn make_invalid_chain_warrant_op(
    keystore: &MetaLairClient,
    warrant_author: AgentPubKey,
    op: &ChainOp,
) -> WorkflowResult<DhtOpHashed> {
    let action = op.action();
    let action_author = action.author().clone();
    tracing::warn!("Authoring warrant for invalid op authored by {action_author}");

    let proof = WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
        action_author: action_author.clone(),
        action: (action.to_hash().clone(), op.signature().clone()),
        chain_op_type: op.get_type(),
    });
    let warrant = Warrant::new(proof, warrant_author, Timestamp::now(), action_author);
    let warrant_op = WarrantOp::sign(keystore, warrant)
        .await
        .map_err(|e| super::WorkflowError::Other(e.into()))?;
    let op: DhtOp = warrant_op.into();
    let op = op.into_hashed();
    Ok(op)
}

pub async fn make_fork_warrant_op_inner(
    keystore: &MetaLairClient,
    warrant_author: AgentPubKey,
    chain_author: AgentPubKey,
    action_pair: ((ActionHash, Signature), (ActionHash, Signature)),
) -> WorkflowResult<DhtOpHashed> {
    debug_assert_ne!(action_pair.0 .0, action_pair.1 .0);
    tracing::warn!(
        "Authoring warrant for chain fork by {chain_author}. Action hashes: ({}, {})",
        action_pair.0 .0,
        action_pair.1 .0
    );

    let warrant = Warrant::new(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
            chain_author: chain_author.clone(),
            action_pair,
        }),
        warrant_author,
        Timestamp::now(),
        chain_author.clone(),
    );
    let warrant_op = WarrantOp::sign(keystore, warrant)
        .await
        .map_err(|e| super::WorkflowError::Other(e.into()))?;
    let op: DhtOp = warrant_op.into();
    let op = op.into_hashed();
    Ok(op)
}

pub fn detect_fork(
    txn: &mut Transaction<'_>,
    action: &Action,
) -> StateQueryResult<Option<(ActionHash, Signature)>> {
    let mut statement = txn.prepare(ACTION_HASH_BY_PREV)?;
    let items = statement
        .query_map(
            named_params! {
                ":prev_hash": action.prev_action(),
                ":hash": action.to_hash(),
            },
            // First, try to deserialize the hash as an ActionHash...
            |row| match row.get("hash") {
                Ok(hash) => {
                    let action_blob: Vec<u8> = row.get("blob")?;
                    Ok(Some((hash, action_blob)))
                }
                // ...if that fails, we can skip it if it deserializes as a WarrantHash
                //    (checking the row type this way makes it so we don't have to join on the DhtOp table in the query)
                Err(err) => match row.get::<_, WarrantHash>("hash") {
                    Ok(_) => Ok(None),
                    Err(_) => Err(err),
                },
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    items
        .into_iter()
        .filter_map(|maybe_tuple| {
            maybe_tuple.map(|(hash, action_blob)| {
                from_blob::<SignedAction>(action_blob)
                    .map(|action| (hash, action.signature().clone()))
            })
        })
        .next()
        .transpose()
}

#[derive(Debug, Clone)]
struct OutcomeSummary {
    accepted: usize,
    missing: usize,
    /// The number of chain ops that were rejected during sys validation
    rejected: usize,
    warranted: usize,
    /// The number of warrant ops that were rejected during sys validation
    warrants_rejected: usize,
    /// The number of warrant dependencies that were copied from the cache to the DHT db
    warrant_deps_copied: usize,
}

impl OutcomeSummary {
    fn new() -> Self {
        OutcomeSummary {
            accepted: 0,
            missing: 0,
            rejected: 0,
            warranted: 0,
            warrants_rejected: 0,
            warrant_deps_copied: 0,
        }
    }
}

impl Default for OutcomeSummary {
    fn default() -> Self {
        OutcomeSummary::new()
    }
}
