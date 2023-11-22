//! The workflow and queue consumer for sys validation

use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::sys_validate::check_and_hold_store_record;
use crate::core::sys_validate::*;
use crate::core::validation::*;
use crate::core::workflow::error::WorkflowResult;
use crate::core::workflow::sys_validation_workflow::validation_batch::{
    validate_ops_batch, NUM_CONCURRENT_OPS,
};
use futures::future;
use futures::future::BoxFuture;
use futures::FutureExt;
use futures::StreamExt;
use holo_hash::DhtOpHash;
use holochain_cascade::error::CascadeResult;
use holochain_cascade::Cascade;
use holochain_cascade::CascadeImpl;
use holochain_cascade::CascadeSource;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use rusqlite::Transaction;
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use tracing::*;
use types::Outcome;

pub mod types;

mod validation_batch;
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

#[instrument(skip(workspace, incoming_dht_ops_sender, trigger_app_validation, network))]
pub async fn sys_validation_workflow<
    Network: HolochainP2pDnaT + Clone + 'static,
    Sender: DhtOpSender + Send + Sync + Clone + 'static,
>(
    workspace: Arc<SysValidationWorkspace>,
    incoming_dht_ops_sender: Sender,
    trigger_app_validation: TriggerSender,
    network: Network,
) -> WorkflowResult<WorkComplete> {
    let complete =
        sys_validation_workflow_inner(workspace, incoming_dht_ops_sender, network).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // trigger other workflows
    trigger_app_validation.trigger(&"sys_validation_workflow");

    Ok(complete)
}

async fn sys_validation_workflow_inner<Network: HolochainP2pDnaT + Clone + 'static>(
    workspace: Arc<SysValidationWorkspace>,
    incoming_dht_ops_sender: impl DhtOpSender + Send + Sync + Clone + 'static,
    network: Network,
) -> WorkflowResult<WorkComplete> {
    let db = workspace.dht_db.clone();
    let sorted_ops = validation_query::get_ops_to_sys_validate(&db).await?;

    if sorted_ops.is_empty() {
        tracing::trace!(
            "Skipping sys_validation_workflow because there are no ops to be validated"
        );
        return Ok(WorkComplete::Complete);
    }

    let num_ops_to_validate = sorted_ops.len();
    tracing::debug!("Validating {} ops", num_ops_to_validate);
    // TODO questionable check for saturation. The query can return up to 10,000 results and this function
    //      will process more than NUM_CONCURRENT_OPS ops, just not in parallel. Not necessarily true that
    //      there will be more ops to process after this workflow run.
    let start = (num_ops_to_validate >= NUM_CONCURRENT_OPS).then(std::time::Instant::now);
    let saturated = start.is_some();
    let cascade = Arc::new(workspace.local_cascade());

    let dna_def = DnaDefHashed::from_content_sync((*workspace.dna_def()).clone());

    // TODO can these clones be eliminated?
    let mut previous_records: ValidationDependencies =
        fetch_previous_records(sorted_ops.clone().into_iter(), cascade.clone()).await;

    // TODO This can now be used instead of the cascade for previous actions
    let mut previous_actions = fetch_previous_actions(
        sorted_ops.iter().map(|op| op.action()),
        cascade.clone(),
        &mut previous_records,
    )
    .await;

    let mut validation_outcomes = Vec::with_capacity(sorted_ops.len());
    // TODO rename
    for hashed_op in sorted_ops {
        let (op, op_hash) = hashed_op.into_inner();
        let op_type = op.get_type();
        let action = op.action();

        // TODO This is more like a 'required dependency' check and isn't actually used in validation
        let dependency = get_dependency(op_type, &action);

        let r = validate_op(
            &op,
            &dna_def,
            cascade.clone(),
            &incoming_dht_ops_sender,
            &mut previous_actions,
        )
        .await;

        match r {
            Ok(outcome) => validation_outcomes.push((op_hash, outcome, dependency)),
            Err(e) => {
                tracing::error!(error = ?e, "Error validating op");
            }
        }
    }

    let summary = workspace
        .dht_db
        .write_async(move |txn| {
            let mut summary = OutcomeSummary::default();
            for (op_hash, outcome, dependency) in validation_outcomes {
                match outcome {
                    Outcome::Accepted => {
                        summary.accepted += 1;
                        put_validation_limbo(txn, &op_hash, ValidationLimboStatus::SysValidated)?;
                    }
                    Outcome::AwaitingOpDep(missing_dep) => {
                        summary.awaiting += 1;
                        // TODO: Try and get this dependency to add to limbo
                        //
                        // I actually can't see how we can do this because there's no
                        // way to get an DhtOpHash without either having the op or the full
                        // action. We have neither that's why where here.
                        //
                        // We need to be holding the dependency because
                        // we were meant to get a StoreRecord or StoreEntry or
                        // RegisterAgentActivity or RegisterAddLink.
                        let status = ValidationLimboStatus::AwaitingSysDeps(missing_dep);
                        put_validation_limbo(txn, &op_hash, status)?;
                    }
                    Outcome::MissingDhtDep => {
                        summary.missing += 1;
                        // TODO: Not sure what missing dht dep is. Check if we need this.
                        put_validation_limbo(txn, &op_hash, ValidationLimboStatus::Pending)?;
                    }
                    Outcome::Rejected => {
                        summary.rejected += 1;
                        if let Dependency::Null = dependency {
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

    // validate_ops_batch(
    //     sorted_ops,
    //     start,
    //     {
    //         let workspace = workspace.clone();
    //         move |so| {
    //             let workspace = workspace.clone();
    //             let cascade = cascade.clone();
    //             let incoming_dht_ops_sender = incoming_dht_ops_sender.clone();
    //             async move {
    //                 let (op, op_hash) = so.into_inner();
    //                 let op_type = op.get_type();
    //                 let action = op.action();

    //                 // TODO This is more like a 'required dependency' check and isn't actually used in validation
    //                 let dependency = get_dependency(op_type, &action);
    //                 let dna_def = DnaDefHashed::from_content_sync((*workspace.dna_def()).clone());

    //                 let r = validate_op(
    //                     &op,
    //                     &dna_def,
    //                     cascade,
    //                     &incoming_dht_ops_sender,
    //                     &mut previous_actions,
    //                 )
    //                 .await;
    //                 r.map(|o| (op_hash, o, dependency))
    //             }
    //             .boxed()
    //         }
    //     },
    //     |batch| {
    //         let workspace = workspace.clone();
    //         async move {
    //             workspace
    //                 .dht_db
    //                 .write_async(move |txn| {
    //                     let mut summary = OutcomeSummary::default();
    //                     for outcome in batch {
    //                         // TODO it's a mistake to bubble here! This error should be handled independently
    //                         let (op_hash, outcome, dependency) = outcome?;
    //                         match outcome {
    //                             Outcome::Accepted => {
    //                                 summary.accepted += 1;
    //                                 put_validation_limbo(
    //                                     txn,
    //                                     &op_hash,
    //                                     ValidationLimboStatus::SysValidated,
    //                                 )?;
    //                             }
    //                             Outcome::AwaitingOpDep(missing_dep) => {
    //                                 summary.awaiting += 1;
    //                                 // TODO: Try and get this dependency to add to limbo
    //                                 //
    //                                 // I actually can't see how we can do this because there's no
    //                                 // way to get an DhtOpHash without either having the op or the full
    //                                 // action. We have neither that's why where here.
    //                                 //
    //                                 // We need to be holding the dependency because
    //                                 // we were meant to get a StoreRecord or StoreEntry or
    //                                 // RegisterAgentActivity or RegisterAddLink.
    //                                 let status =
    //                                     ValidationLimboStatus::AwaitingSysDeps(missing_dep);
    //                                 put_validation_limbo(txn, &op_hash, status)?;
    //                             }
    //                             Outcome::MissingDhtDep => {
    //                                 summary.missing += 1;
    //                                 // TODO: Not sure what missing dht dep is. Check if we need this.
    //                                 put_validation_limbo(
    //                                     txn,
    //                                     &op_hash,
    //                                     ValidationLimboStatus::Pending,
    //                                 )?;
    //                             }
    //                             Outcome::Rejected => {
    //                                 summary.rejected += 1;
    //                                 if let Dependency::Null = dependency {
    //                                     put_integrated(txn, &op_hash, ValidationStatus::Rejected)?;
    //                                 } else {
    //                                     put_integration_limbo(
    //                                         txn,
    //                                         &op_hash,
    //                                         ValidationStatus::Rejected,
    //                                     )?;
    //                                 }
    //                             }
    //                         }
    //                     }
    //                     WorkflowResult::Ok(summary)
    //                 })
    //                 .await
    //         }
    //         .boxed()
    //     },
    // )
    // .await?;

    Ok(if saturated {
        WorkComplete::Incomplete(None)
    } else {
        WorkComplete::Complete
    })
}

pub(crate) struct ValidationDependencies {
    dependencies: HashMap<AnyDhtHash, Option<ValidationDependencyState>>,
}

impl ValidationDependencies {
    fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
        }
    }

    fn has(&self, hash: &AnyDhtHash) -> bool {
        self.dependencies.contains_key(hash)
    }

    fn get(&mut self, hash: &AnyDhtHash) -> Option<&mut ValidationDependencyState> {
        match self.dependencies.get_mut(hash) {
            Some(Some(dep)) => Some(dep),
            Some(None) => None,
            None => {
                tracing::warn!(hash = ?hash, "Have not attempted to fetch requested dependency, this is a bug");
                None
            }
        }
    }
}

impl FromIterator<(AnyDhtHash, Option<ValidationDependencyState>)> for ValidationDependencies {
    fn from_iter<T: IntoIterator<Item = (AnyDhtHash, Option<ValidationDependencyState>)>>(
        iter: T,
    ) -> Self {
        Self {
            dependencies: iter.into_iter().collect(),
        }
    }
}

struct ValidationDependencyState {
    dependency: ValidationDependency,
    fetched_from: CascadeSource,
}

impl ValidationDependencyState {
    fn as_action(&self) -> &Action {
        match &self.dependency {
            ValidationDependency::Action(signed_action) => signed_action.action(),
            ValidationDependency::Record(record) => record.action(),
        }
    }

    fn as_record(&self) -> Option<&Record> {
        match &self.dependency {
            ValidationDependency::Action(_) => {
                tracing::warn!("Attempted to get a record from a dependency that is an action, this is a bug");
                None
            },
            ValidationDependency::Record(record) => Some(record),
        }
    }
}

enum ValidationDependency {
    Action(SignedActionHashed),
    Record(Record),
}

impl From<(SignedActionHashed, CascadeSource)> for ValidationDependencyState {
    fn from((signed_action, fetched_from): (SignedActionHashed, CascadeSource)) -> Self {
        Self {
            dependency: ValidationDependency::Action(signed_action),
            fetched_from,
        }
    }
}

impl From<(Record, CascadeSource)> for ValidationDependencyState {
    fn from((record, fetched_from): (Record, CascadeSource)) -> Self {
        Self {
            dependency: ValidationDependency::Record(record),
            fetched_from,
        }
    }
}

async fn fetch_previous_actions<A, C>(
    actions: A,
    cascade: Arc<C>,
    previous_records: &ValidationDependencies,
) -> ValidationDependencies
where
    A: Iterator<Item = Action>,
    C: Cascade + Send + Sync,
{
    let action_fetches = actions.into_iter().flat_map(|action| {
        // For each previous action that will be needed for validation, map the action to a fetch Action for its hash
        vec![
            match &action {
                Action::Update(update) => Some(update.original_action_address.clone()),
                _ => None,
            },
            match action.prev_action().cloned() {
                None => None,
                hash => hash,
            },
        ]
        .into_iter()
        .filter_map(|hash| {
            let cascade = cascade.clone();
            match hash {
                Some(h) => {
                    // Skip fetching the action because we already have the Record in memory which contains the action anyway.
                    if previous_records.has(&h.clone().into()) {
                        None
                    } else {
                        Some(
                            async move {
                                let fetched =
                                    cascade.retrieve_action(h.clone(), Default::default()).await;
                                (h, fetched)
                            }
                            .boxed(),
                        )
                    }
                }
                None => None,
            }
        })
    });

    futures::future::join_all(action_fetches)
        .await
        .into_iter()
        .filter_map(|r| {
            // Filter out errors, preparing the rest to be put into a HashMap for easy access.
            match r {
                (hash, Ok(Some((signed_action, source)))) => {
                    Some((hash.into(), Some((signed_action, source).into())))
                }
                (hash, Ok(None)) => {
                    Some((hash.into(), None))
                },
                (hash, Err(e)) => {
                    tracing::error!(error = ?e, action_hash = ?hash, "Error retrieving prev action");
                    None
                }
            }
        })
        .collect()
}

async fn fetch_previous_records<O, C>(ops: O, cascade: Arc<C>) -> ValidationDependencies
where
    O: Iterator<Item = DhtOpHashed>,
    C: Cascade + Send + Sync,
{
    let action_fetches = ops.into_iter().flat_map(|op| {
        // For each previous action that will be needed for validation, map the action to a fetch Record for its hash
        vec![match &op.content {
            DhtOp::RegisterAgentActivity(_, action) => {
                action.prev_action().map(|a| a.as_hash().clone().into())
            }
            DhtOp::RegisterUpdatedContent(_, action, _) => {
                Some(action.original_action_address.clone().into())
            }
            DhtOp::RegisterUpdatedRecord(_, action, _) => {
                Some(action.original_action_address.clone().into())
            }
            DhtOp::RegisterDeletedBy(_, action) => {
                Some(action.deletes_address.clone().into())
            }
            DhtOp::RegisterDeletedEntryAction(_, action) => {
                Some(action.deletes_address.clone().into())
            }
            DhtOp::RegisterRemoveLink(_, action) => {
                Some(action.link_add_address.clone().into())
            }
            _ => None,
        }]
        .into_iter()
        .filter_map(|hash: Option<AnyDhtHash>| {
            let cascade = cascade.clone();
            match hash {
                Some(h) => Some(
                    async move {
                        let fetched = cascade.retrieve(h.clone(), Default::default()).await;
                        (h, fetched)
                    }
                    .boxed(),
                ),
                None => None,
            }
        })
    });

    futures::future::join_all(action_fetches)
        .await
        .into_iter()
        .filter_map(|r| {
            // Filter out errors, preparing the rest to be put into a HashMap for easy access.
            match r {
                (hash, Ok(Some((record, CascadeSource::Local)))) => {
                    Some((hash.into(), Some((record, CascadeSource::Local).into())))
                }
                (hash, Ok(Some((record, CascadeSource::Network)))) => {
                    Some((hash.into(), Some((record.privatized().0, CascadeSource::Network).into())))
                }
                (hash, Ok(None)) => {
                    Some((hash.into(), None))
                },
                (hash, Err(e)) => {
                    tracing::error!(error = ?e, action_hash = ?hash, "Error retrieving prev action");
                    None
                }
            }
        })
        .collect()
}

/// Validate a single DhtOp, using the supplied Cascade to draw dependencies from
pub(crate) async fn validate_op<C>(
    op: &DhtOp,
    dna_def: &DnaDefHashed,
    cascade: Arc<C>,
    incoming_dht_ops_sender: &impl DhtOpSender,
    validation_dependencies: &mut ValidationDependencies,
) -> WorkflowResult<Outcome>
where
    C: Cascade + Send + Sync,
{
    match validate_op_inner(
        op,
        cascade,
        dna_def,
        incoming_dht_ops_sender,
        validation_dependencies,
    )
    .await
    {
        Ok(_) => Ok(Outcome::Accepted),
        // Handle the errors that result in pending or awaiting deps
        Err(SysValidationError::ValidationOutcome(e)) => {
            info!(
                msg = "DhtOp did not pass system validation. (If rejected, a warning will follow.)",
                ?op,
                error = ?e,
                error_msg = %e
            );
            let outcome = handle_failed(&e);
            if let Outcome::Rejected = outcome {
                warn!(msg = "DhtOp was rejected during system validation.", ?op, error = ?e, error_msg = %e)
            }
            Ok(outcome)
        }
        Err(e) => Err(e.into()), // TODO questionable conversion, the state of one op validation should not determine the workflow result
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
        ValidationOutcome::ActionNotInCounterSigningSession(_, _) => Rejected,
        ValidationOutcome::DepMissingFromDht(_) => MissingDhtDep,
        ValidationOutcome::EntryDefId(_) => Rejected,
        ValidationOutcome::EntryHash => Rejected,
        ValidationOutcome::EntryTooLarge(_) => Rejected,
        ValidationOutcome::EntryTypeMismatch => Rejected,
        ValidationOutcome::EntryVisibility(_) => Rejected,
        ValidationOutcome::TagTooLarge(_) => Rejected,
        ValidationOutcome::MalformedDhtOp(_, _, _) => Rejected,
        ValidationOutcome::NotCreateLink(_) => Rejected,
        ValidationOutcome::NotNewEntry(_) => Rejected,
        // TODO The only place we mark as waiting for another op to be validated and it's not correct?
        ValidationOutcome::NotHoldingDep(dep) => AwaitingOpDep(dep.clone()),
        ValidationOutcome::PrevActionError(_) => Rejected,
        ValidationOutcome::PrivateEntryLeaked => Rejected,
        ValidationOutcome::PreflightResponseSignature(_) => Rejected,
        ValidationOutcome::UpdateTypeMismatch(_, _) => Rejected,
        ValidationOutcome::UpdateHashMismatch(_, _) => Rejected,
        ValidationOutcome::VerifySignature(_, _) => Rejected,
        ValidationOutcome::WrongDna(_, _) => Rejected,
        ValidationOutcome::ZomeIndex(_) => Rejected,
        ValidationOutcome::CounterSigningError(_) => Rejected,
    }
}

async fn validate_op_inner<C>(
    op: &DhtOp,
    cascade: Arc<C>,
    dna_def: &DnaDefHashed,
    incoming_dht_ops_sender: &impl DhtOpSender,
    validation_dependencies: &mut ValidationDependencies,
) -> SysValidationResult<()>
where
    C: Cascade + Send + Sync,
{
    check_entry_visibility(op)?;
    match op {
        DhtOp::StoreRecord(_, action, entry) => {
            store_record(action, validation_dependencies)?;
            if let Some(entry) = entry.as_option() {
                // Retrieve for all other actions on countersigned entry.
                if let Entry::CounterSign(session_data, _) = entry {
                    let entry_hash = EntryHash::with_data_sync(entry);
                    let weight = action
                        .entry_rate_data()
                        .ok_or_else(|| SysValidationError::NonEntryAction(action.clone()))?;
                    for action in session_data.build_action_set(entry_hash, weight)? {
                        let hh = ActionHash::with_data_sync(&action);
                        // TODO eliminate me
                        if cascade
                            .retrieve_action(hh.clone(), Default::default())
                            .await?
                            .is_none()
                        {
                            return Err(SysValidationError::ValidationOutcome(
                                ValidationOutcome::DepMissingFromDht(hh.into()),
                            ));
                        }
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
                let dependency_check = |_original_record: &Record| Ok(());
                let entry_hash = EntryHash::with_data_sync(entry);
                let weight = match action {
                    NewEntryAction::Create(h) => h.weight.clone(),
                    NewEntryAction::Update(h) => h.weight.clone(),
                };
                for action in session_data.build_action_set(entry_hash, weight)? {
                    check_and_hold_store_record(
                        &ActionHash::with_data_sync(&action),
                        cascade.clone(),
                        Some(incoming_dht_ops_sender),
                        dependency_check,
                    )
                    .await?;
                }
            }

            store_entry((action).into(), entry, validation_dependencies).await?;

            let action = action.clone().into();
            store_record(&action, validation_dependencies)
        }
        DhtOp::RegisterAgentActivity(_, action) => {
            register_agent_activity(action, validation_dependencies, dna_def)?;
            store_record(action, validation_dependencies)
        }
        DhtOp::RegisterUpdatedContent(_, action, entry) => {
            register_updated_content(action, validation_dependencies)?;
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
            register_updated_record(action, validation_dependencies)?;
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
        DhtOp::RegisterDeletedBy(_, action) => {
            register_deleted_by(action, validation_dependencies)
        }
        DhtOp::RegisterDeletedEntryAction(_, action) => {
            register_deleted_entry_action(action, validation_dependencies)
        }
        DhtOp::RegisterAddLink(_, action) => {
            register_add_link(action)
        }
        DhtOp::RegisterRemoveLink(_, action) => {
            register_delete_link(action, validation_dependencies)
        }
    }
}

// #[instrument(skip(record, call_zome_workspace, network))]
/// Direct system validation call that takes
/// a Record instead of an op.
/// Does not require holding dependencies.
/// Will not await dependencies and instead returns
/// that outcome immediately.
pub async fn sys_validate_record<C>(record: &Record, cascade: Arc<C>) -> SysValidationOutcome<()>
where
    C: Cascade + Send + Sync,
{
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

async fn sys_validate_record_inner<C>(record: &Record, cascade: Arc<C>) -> SysValidationResult<()>
where
    C: Cascade + Send + Sync,
{
    let signature = record.signature();
    let action = record.action();
    let maybe_entry = record.entry().as_option();
    counterfeit_check(signature, action).await?;

    async fn validate<C>(
        action: &Action,
        maybe_entry: Option<&Entry>,
        cascade: Arc<C>,
    ) -> SysValidationResult<()>
    where
        C: Cascade + Send + Sync,
    {
        let mut validation_dependencies =
            fetch_previous_actions(vec![action.clone()].into_iter(), cascade.clone(), &ValidationDependencies::new()).await;

        store_record(action, &mut validation_dependencies)?;
        if let Some(maybe_entry) = maybe_entry {
            store_entry(
                action
                    .try_into()
                    .map_err(|_| ValidationOutcome::NotNewEntry(action.clone()))?,
                maybe_entry,
                &mut validation_dependencies,
            )
            .await?;
        }
        match action {
            Action::Update(action) => {
                register_updated_content(action, &mut validation_dependencies)
            }
            Action::Delete(action) => {
                register_deleted_entry_action(action, &mut validation_dependencies)
            }
            Action::CreateLink(action) => {
                register_add_link(action)
            }
            Action::DeleteLink(action) => {
                register_delete_link(action, &mut validation_dependencies)
            }
            _ => {
                Ok(())
            }
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
    validation_dependencies: &mut ValidationDependencies,
    dna_def: &DnaDefHashed,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let prev_action_hash = action.prev_action();

    // Checks
    check_prev_action(action)?;
    check_valid_if_dna(action, dna_def)?;
    if let Some(prev_action_hash) = prev_action_hash {
        // Just make sure we have the dependency and if not then don't mark this action as valid yet
        validation_dependencies.get(&prev_action_hash.clone().into()).ok_or_else(|| {
            ValidationOutcome::DepMissingFromDht(prev_action_hash.clone().into())
        })?;
    }

    Ok(())
}

fn store_record(
    action: &Action,
    validation_dependencies: &mut ValidationDependencies,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let prev_action_hash = action.prev_action();

    // Checks
    check_prev_action(action)?;
    if let Some(prev_action_hash) = prev_action_hash {
        let state = validation_dependencies
            .get(&prev_action_hash.clone().into())
            .ok_or_else(|| ValidationOutcome::DepMissingFromDht(prev_action_hash.clone().into()))?;
        let prev_action = state.as_action();
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
    validation_dependencies: &mut ValidationDependencies,
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
        let state = validation_dependencies
            .get(&original_action_address.clone().into())
            .ok_or_else(|| {
                ValidationOutcome::DepMissingFromDht(original_action_address.clone().into())
            })?;
        let original_action = state.as_action();
        // TODO verify previous has passed sys validation?
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
    validation_dependencies: &mut ValidationDependencies,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_action_address = &entry_update.original_action_address;

    let state = validation_dependencies.get(&original_action_address.clone().into()).ok_or_else(|| {
        ValidationOutcome::DepMissingFromDht(original_action_address.clone().into())
    })?;

    update_check(entry_update, state.as_action())
}

fn register_updated_record(
    entry_update: &Update,
    validation_dependencies: &mut ValidationDependencies,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_action_address = &entry_update.original_action_address;

    let state = validation_dependencies.get(&original_action_address.clone().into()).ok_or_else(|| {
        ValidationOutcome::DepMissingFromDht(original_action_address.clone().into())
    })?;

    update_check(entry_update, state.as_action())
}

fn register_deleted_by(
    record_delete: &Delete,
    validation_dependencies: &mut ValidationDependencies,
) -> SysValidationResult<()>
{
    // Get data ready to validate
    let removed_action_address = &record_delete.deletes_address;

    let state = validation_dependencies.get(&removed_action_address.clone().into()).ok_or_else(|| {
        ValidationOutcome::DepMissingFromDht(removed_action_address.clone().into())
    })?;

    check_new_entry_action(state.as_action())
}

fn register_deleted_entry_action(
    record_delete: &Delete,
    validation_dependencies: &mut ValidationDependencies,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_action_address = &record_delete.deletes_address;

    let state = validation_dependencies.get(&removed_action_address.clone().into()).ok_or_else(|| {
        ValidationOutcome::DepMissingFromDht(removed_action_address.clone().into())
    })?;

    check_new_entry_action(state.as_action())
}

fn register_add_link(
    link_add: &CreateLink,
) -> SysValidationResult<()> {
    check_tag_size(&link_add.tag)
}

fn register_delete_link(
    link_remove: &DeleteLink,
    validation_dependencies: &mut ValidationDependencies
) -> SysValidationResult<()> {
    // Get data ready to validate
    let link_add_address = &link_remove.link_add_address;

    // Just require that this link exists, don't need to check anything else about it here
    validation_dependencies.get(&link_add_address.clone().into()).ok_or_else(|| {
        ValidationOutcome::DepMissingFromDht(link_add_address.clone().into())
    })?;

    Ok(())
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
}

impl SysValidationWorkspace {
    pub fn new(
        authored_db: DbRead<DbKindAuthored>,
        dht_db: DbWrite<DbKindDht>,
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
        let cascade = CascadeImpl::empty().with_dht(self.dht_db.clone().into());
        match &self.scratch {
            Some(scratch) => cascade
                .with_authored(self.authored_db.clone())
                .with_scratch(scratch.clone()),
            None => cascade,
        }
    }

    /// Create a cascade with access to local data as well as network data
    pub fn full_cascade<Network: HolochainP2pDnaT + Clone + 'static + Send>(
        &self,
        network: Network,
    ) -> CascadeImpl<Network> {
        let cascade = CascadeImpl::empty()
            .with_dht(self.dht_db.clone().into())
            .with_network(network, self.cache.clone());
        match &self.scratch {
            Some(scratch) => cascade
                .with_authored(self.authored_db.clone())
                .with_scratch(scratch.clone()),
            None => cascade,
        }
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

struct OutcomeSummary {
    accepted: usize,
    awaiting: usize,
    missing: usize,
    rejected: usize,
}

impl OutcomeSummary {
    fn new() -> Self {
        OutcomeSummary {
            accepted: 0,
            awaiting: 0,
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
