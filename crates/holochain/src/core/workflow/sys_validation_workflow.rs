//! The workflow and queue consumer for sys validation

use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::core::sys_validate::*;
use crate::core::validation::*;
use crate::core::workflow::error::WorkflowResult;
use crate::core::workflow::sys_validation_workflow::validation_batch::NUM_CONCURRENT_OPS;
use futures::FutureExt;
use holo_hash::DhtOpHash;
use holochain_cascade::Cascade;
use holochain_cascade::CascadeImpl;
use holochain_cascade::CascadeSource;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use parking_lot::Mutex;
use rusqlite::Transaction;
use std::collections::HashMap;
use std::collections::HashSet;
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

#[instrument(skip(
    workspace,
    incoming_dht_ops_sender,
    current_validation_dependencies,
    trigger_app_validation,
    trigger_self,
    network
))]
pub async fn sys_validation_workflow<
    Network: HolochainP2pDnaT + Clone + 'static,
    Sender: DhtOpSender + Send + Sync + Clone + 'static,
>(
    workspace: Arc<SysValidationWorkspace>,
    incoming_dht_ops_sender: Sender,
    current_validation_dependencies: Arc<Mutex<ValidationDependencies>>,
    trigger_app_validation: TriggerSender,
    trigger_self: TriggerSender,
    network: Network,
) -> WorkflowResult<WorkComplete> {
    // Run the actual sys validation using data we have locally
    let complete =
        sys_validation_workflow_inner(workspace.clone(), current_validation_dependencies.clone())
            .await?;

    // TODO remove WorkComplete above and return stats to here to help decide whether to proceed.
    // trigger app validation to process any ops that have been processed so far
    trigger_app_validation.trigger(&"sys_validation_workflow");

    // Now go to the network to try to fetch missing dependencies
    let network_cascade = Arc::new(workspace.network_and_cache_cascade(network));
    let missing_action_hashes = current_validation_dependencies.lock().get_missing_hashes();
    // TODO concurrency should be bounded here
    let num_fetched: usize =
        futures::future::join_all(missing_action_hashes.into_iter().map(|hash| {
            let network_cascade = network_cascade.clone();
            let current_validation_dependencies = current_validation_dependencies.clone();
            async move {
                match network_cascade.retrieve(hash, Default::default()).await {
                    Ok(Some((record, source))) => {
                        let mut deps = current_validation_dependencies.lock();

                        // If the source was local then that means some other fetch has put this action into the cache,
                        // that's fine we'll just grab it here.
                        if deps.insert(record.signed_action, source) {
                            1
                        } else {
                            0
                        }
                    }
                    Ok(None) => {
                        // This is fine, we didn't find it on the network, so we'll have to try again.
                        // TODO put this on a timeout to avoid hitting the network too often for it?
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
        .await
        .into_iter()
        .sum();

    // retrigger this workflow because we fetched some missing dependencies from the network.
    if num_fetched > 0 {
        tracing::debug!(num_fetched = ?num_fetched, "Fetched missing dependencies from the network, retrying sys validation");
        trigger_self.trigger(&"sys_validation_workflow");
    }

    // Finally, for anything we've fetched from the network we need to push it into the incoming dht ops workflow
    let network_fetched_action_hashes = current_validation_dependencies
        .lock()
        .get_network_fetched_hashes();
    futures::future::join_all(network_fetched_action_hashes.into_iter().map(|hash| {
        let current_validation_dependencies = current_validation_dependencies.clone();
        let incoming_dht_ops_sender = incoming_dht_ops_sender.clone();
        async move {
            let state = {
                let mut deps = current_validation_dependencies.lock();
                let mut state = deps.get(&hash);
                if let Some(dep) = state.as_mut().and_then(|s| s.dependency.as_mut()) {
                    match dep {
                        ValidationDependency::Action(_, source) => {
                            *source = CascadeSource::Local;
                        }
                        ValidationDependency::Record(_, source) => {
                            *source = CascadeSource::Local;
                        }
                    }
                }

                state.cloned()
            };

            if let Some(state) = state {
                let op_type = state.required_by_op_type.clone();
                let record = state.as_record().cloned();

                if let (Some(op_type), Some(record)) = (op_type, record) {
                    let result = match op_type {
                        DhtOpType::StoreRecord => {
                            incoming_dht_ops_sender
                                .send_store_record(
                                    record,
                                )
                                .await
                        }
                        DhtOpType::StoreEntry => {
                            incoming_dht_ops_sender
                                .send_store_entry(
                                    record,
                                )
                                .await
                        }
                        DhtOpType::RegisterAgentActivity => {
                            incoming_dht_ops_sender
                                .send_register_agent_activity(
                                    record,
                                )
                                .await
                        }
                        DhtOpType::RegisterAddLink => {
                            incoming_dht_ops_sender
                                .send_register_add_link(
                                    record,
                                )
                                .await
                        }
                        _ => {
                            tracing::warn!(op_type = ?op_type, "Unexpected op type for network fetched dependency");
                            Ok(())
                        }
                    };

                    if let Err(e) = result {
                        tracing::error!(error = ?e, "Error sending network fetched dependency to incoming dht ops");
                    }
                }
            }
        }
        .boxed()
    }))
    .await;

    Ok(complete)
}

async fn sys_validation_workflow_inner(
    workspace: Arc<SysValidationWorkspace>,
    current_validation_dependencies: Arc<Mutex<ValidationDependencies>>,
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

    // Forget what dependencies are currently in use
    current_validation_dependencies.lock().clear_retained_deps();

    // TODO can these clones be eliminated?
    fetch_previous_records(
        current_validation_dependencies.clone(),
        cascade.clone(),
        sorted_ops.clone().into_iter(),
    )
    .await;
    fetch_previous_actions(
        current_validation_dependencies.clone(),
        cascade.clone(),
        sorted_ops.iter().map(|op| op.action()),
    )
    .await;

    // Now drop all the dependencies that we didn't just try to access while searching the current set of ops to validate.
    current_validation_dependencies.lock().purge_held_deps();

    let mut validation_outcomes = Vec::with_capacity(sorted_ops.len());
    for hashed_op in sorted_ops {
        let (op, op_hash) = hashed_op.into_inner();
        let op_type = op.get_type();
        let action = op.action();

        // TODO This is more like a 'required dependency' check and isn't actually used in validation
        let dependency = get_dependency(op_type, &action);

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

    tracing::info!(
        ?summary,
        ?saturated,
        ?num_ops_to_validate,
        "Finished sys validation workflow"
    );

    Ok(if saturated {
        WorkComplete::Incomplete(None)
    } else {
        WorkComplete::Complete
    })
}

pub struct ValidationDependencies {
    states: HashMap<AnyDhtHash, ValidationDependencyState>,
    retained_deps: HashSet<AnyDhtHash>,
}

impl Default for ValidationDependencies {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationDependencies {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            retained_deps: HashSet::new(),
        }
    }

    pub fn has(&mut self, hash: &AnyDhtHash) -> bool {
        self.retained_deps.insert(hash.clone());
        self.states
            .get(hash)
            .map(|state| state.dependency.is_some())
            .unwrap_or(false)
    }

    pub fn get(&mut self, hash: &AnyDhtHash) -> Option<&mut ValidationDependencyState> {
        match self.states.get_mut(hash) {
            Some(dep) => Some(dep),
            None => {
                tracing::warn!(hash = ?hash, "Have not attempted to fetch requested dependency, this is a bug");
                None
            }
        }
    }

    fn get_missing_hashes(&self) -> Vec<AnyDhtHash> {
        self.states
            .iter()
            .filter_map(|(hash, state)| {
                if state.dependency.is_none() {
                    Some(hash.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_network_fetched_hashes(&self) -> Vec<AnyDhtHash> {
        self.states
            .iter()
            .filter_map(|(hash, state)| match state {
                ValidationDependencyState {
                    dependency: Some(ValidationDependency::Action(_, CascadeSource::Network)),
                    ..
                }
                | ValidationDependencyState {
                    dependency: Some(ValidationDependency::Record(_, CascadeSource::Network)),
                    ..
                } => Some(hash.clone()),
                _ => None,
            })
            .collect()
    }

    fn insert(&mut self, action: SignedActionHashed, source: CascadeSource) -> bool {
        let hash: AnyDhtHash = action.as_hash().clone().into();
        if self.has(&hash) {
            tracing::warn!(hash = ?hash, "Attempted to insert a dependency that was already present, this is not expected");
            return false;
        }
        self.retained_deps.insert(hash.clone());
        self.states.insert(hash, (action, source).into());

        true
    }

    fn clear_retained_deps(&mut self) {
        self.retained_deps.clear();
    }

    fn purge_held_deps(&mut self) {
        self.states.retain(|k, _| self.retained_deps.contains(k));
    }

    // TODO too simple a merge? We don't expect to have any duplicates in the two sets because we
    //      filter before we fetch.
    fn merge(&mut self, other: Self) {
        self.retained_deps.extend(other.states.keys().cloned());
        self.states.extend(other.states);
    }
}

impl FromIterator<(AnyDhtHash, ValidationDependencyState)> for ValidationDependencies {
    fn from_iter<T: IntoIterator<Item = (AnyDhtHash, ValidationDependencyState)>>(iter: T) -> Self {
        Self {
            states: iter.into_iter().collect(),
            retained_deps: HashSet::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ValidationDependencyState {
    dependency: Option<ValidationDependency>,
    // The type of the op that referenced this dependency
    required_by_op_type: Option<DhtOpType>,
}

impl ValidationDependencyState {
    fn as_action(&self) -> Option<&Action> {
        match &self.dependency {
            Some(ValidationDependency::Action(signed_action, _)) => Some(signed_action.action()),
            Some(ValidationDependency::Record(record, _)) => Some(record.action()),
            None => {
                tracing::warn!("Attempted to get an action from a dependency that is None");
                None
            }
        }
    }

    fn as_record(&self) -> Option<&Record> {
        match &self.dependency {
            Some(ValidationDependency::Action(_, _)) => {
                tracing::warn!(
                    "Attempted to get a record from a dependency that is an action, this is a bug"
                );
                None
            }
            Some(ValidationDependency::Record(record, _)) => Some(record),
            None => None,
        }
    }
}

#[derive(Clone, Debug)]
enum ValidationDependency {
    Action(SignedActionHashed, CascadeSource),
    Record(Record, CascadeSource),
}

impl From<(SignedActionHashed, CascadeSource)> for ValidationDependencyState {
    fn from((signed_action, fetched_from): (SignedActionHashed, CascadeSource)) -> Self {
        Self {
            dependency: Some(ValidationDependency::Action(signed_action, fetched_from)),
            required_by_op_type: None,
        }
    }
}

impl From<(Record, CascadeSource, DhtOpType)> for ValidationDependencyState {
    fn from((record, fetched_from, op_type): (Record, CascadeSource, DhtOpType)) -> Self {
        Self {
            dependency: Some(ValidationDependency::Record(record, fetched_from)),
            required_by_op_type: Some(op_type),
        }
    }
}

async fn fetch_previous_actions<A, C>(
    current_validation_dependencies: Arc<Mutex<ValidationDependencies>>,
    cascade: Arc<C>,
    actions: A,
) where
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
        // TODO resolve differences with record fetch
        .filter_map(|hash| {
            let cascade = cascade.clone();
            match hash {
                Some(h) => {
                    // Skip fetching the action because we already have the Record in memory which contains the action anyway.
                    if current_validation_dependencies
                        .lock()
                        .has(&h.clone().into())
                    {
                        tracing::info!(hash = ?h, "Already have action for validation");
                        None
                    } else {
                        Some(
                            async move {
                                let fetched =
                                    cascade.retrieve_action(h.clone(), Default::default()).await;
                                tracing::info!(hash = ?h, fetched = ?fetched, "Fetched action for validation");
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

    let new_deps: ValidationDependencies = futures::future::join_all(action_fetches)
        .await
        .into_iter()
        .filter_map(|r| {
            // Filter out errors, preparing the rest to be put into a HashMap for easy access.
            match r {
                (hash, Ok(Some((signed_action, source)))) => {
                    Some((hash.into(), (signed_action, source).into()))
                }
                (hash, Ok(None)) => {
                    Some((hash.into(), ValidationDependencyState {
                        dependency: None,
                        required_by_op_type: None,
                    }))
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

async fn fetch_previous_records<C, O>(
    current_validation_dependencies: Arc<Mutex<ValidationDependencies>>,
    cascade: Arc<C>,
    ops: O,
) where
    C: Cascade + Send + Sync,
    O: Iterator<Item = DhtOpHashed>,
{
    let action_fetches = ops.into_iter().flat_map(|op| {
        // For each previous action that will be needed for validation, map the action to a fetch Record for its hash
        match &op.content {
            DhtOp::StoreRecord(_, action, RecordEntry::Present(entry)) => {
                match entry {
                    Entry::CounterSign(session_data, _) => {
                        // Discard errors here because we'll check later whether the input is valid. If it's not then it
                        // won't matter that we've skipped fetching deps for it
                        if let Ok(entry_rate_weight) = action_to_entry_rate_weight(action) {
                            make_action_set_for_session_data(entry_rate_weight, entry, session_data)
                                .ok()
                                .map(|actions| {
                                    actions
                                        .into_iter()
                                        .map(|a| (a.into(), op.get_type()))
                                        .collect()
                                })
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            DhtOp::StoreEntry(_, action, entry) => {
                match entry {
                    Entry::CounterSign(session_data, _) => {
                        // Discard errors here because we'll check later whether the input is valid. If it's not then it
                        // won't matter that we've skipped fetching deps for it
                        make_action_set_for_session_data(
                            new_entry_action_to_entry_rate_weight(action),
                            entry,
                            &session_data,
                        )
                        .ok()
                        .map(|actions| {
                            actions
                                .into_iter()
                                .map(|a| (a.into(), op.get_type()))
                                .collect()
                        })
                    }
                    _ => None,
                }
            }
            DhtOp::RegisterUpdatedContent(_, action, _) => Some(vec![(
                action.original_action_address.clone().into(),
                op.get_type(),
            )]),
            DhtOp::RegisterUpdatedRecord(_, action, _) => Some(vec![(
                action.original_action_address.clone().into(),
                op.get_type(),
            )]),
            DhtOp::RegisterDeletedBy(_, action) => {
                Some(vec![(action.deletes_address.clone().into(), op.get_type())])
            }
            DhtOp::RegisterDeletedEntryAction(_, action) => {
                Some(vec![(action.deletes_address.clone().into(), op.get_type())])
            }
            DhtOp::RegisterRemoveLink(_, action) => Some(vec![(
                action.link_add_address.clone().into(),
                op.get_type(),
            )]),
            _ => None,
        }
        .into_iter()
        .flatten()
        .filter(|(hash, _)| !current_validation_dependencies.lock().has(hash))
        .map(|(hash, op_type): (AnyDhtHash, DhtOpType)| {
            let cascade = cascade.clone();
            async move {
                let fetched = cascade.retrieve(hash.clone(), Default::default()).await;
                (hash, op_type, fetched)
            }
            .boxed()
        })
    });

    let new_deps = futures::future::join_all(action_fetches)
        .await
        .into_iter()
        .filter_map(|r| {
            // Filter out errors, preparing the rest to be put into a HashMap for easy access.
            match r {
                (hash, op_type, Ok(Some((record, CascadeSource::Local)))) => {
                    Some((hash.into(), (record, CascadeSource::Local, op_type).into()))
                }
                (hash, op_type, Ok(Some((record, CascadeSource::Network)))) => {
                    Some((hash.into(), (record.privatized().0, CascadeSource::Network, op_type).into()))
                }
                (hash, op_type, Ok(None)) => {
                    Some((hash.into(), ValidationDependencyState {
                        dependency: None,
                        required_by_op_type: Some(op_type),
                    }))
                },
                (hash, _, Err(e)) => {
                    tracing::error!(error = ?e, action_hash = ?hash, "Error retrieving prev action");
                    None
                }
            }
        })
        .collect();

    current_validation_dependencies.lock().merge(new_deps);
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
    session_data: &Box<CounterSigningSessionData>,
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
            store_record(action, validation_dependencies.clone())?;
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
                            .get(&action_hash.clone().into())
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
                        .get(&action_hash.clone().into())
                        .and_then(|s| s.as_action())
                        .ok_or_else(|| {
                            ValidationOutcome::DepMissingFromDht(action_hash.clone().into())
                        })?;
                }
            }

            store_entry((action).into(), entry, validation_dependencies.clone()).await?;

            let action = action.clone().into();
            store_record(&action, validation_dependencies)
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
            .get(&prev_action_hash.clone().into())
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
            .get(&prev_action_hash.clone().into())
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
            .get(&original_action_address.clone().into())
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
        .get(&original_action_address.clone().into())
        .and_then(|s| s.as_action())
        .ok_or_else(|| {
            ValidationOutcome::DepMissingFromDht(original_action_address.clone().into())
        })?;

    update_check(entry_update, original_action)
}

fn register_updated_record(
    entry_update: &Update,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let original_action_address = &entry_update.original_action_address;

    let mut validation_dependencies = validation_dependencies.lock();
    let original_action = validation_dependencies
        .get(&original_action_address.clone().into())
        .and_then(|s| s.as_action())
        .ok_or_else(|| {
            ValidationOutcome::DepMissingFromDht(original_action_address.clone().into())
        })?;

    update_check(entry_update, original_action)
}

fn register_deleted_by(
    record_delete: &Delete,
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
) -> SysValidationResult<()> {
    // Get data ready to validate
    let removed_action_address = &record_delete.deletes_address;

    let mut validation_dependencies = validation_dependencies.lock();
    let action = validation_dependencies
        .get(&removed_action_address.clone().into())
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
        .get(&removed_action_address.clone().into())
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
    validation_dependencies
        .get(&link_add_address.clone().into())
        .and_then(|s| s.as_action())
        .ok_or_else(|| ValidationOutcome::DepMissingFromDht(link_add_address.clone().into()))?;

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
        let cascade = CascadeImpl::empty()
            .with_dht(self.dht_db.clone().into())
            .with_cache(self.cache.clone().into());
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

#[derive(Debug, Clone)]
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
