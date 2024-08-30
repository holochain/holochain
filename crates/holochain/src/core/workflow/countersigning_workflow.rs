//! Countersigning workflow to maintain countersigning session state.

#![allow(unused)]

use super::error::WorkflowResult;
use crate::conductor::space::Space;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::{QueueTriggers, TriggerSender, WorkComplete};
use crate::core::ribosome::weigh_placeholder;
use crate::core::workflow::WorkflowError;
use holo_hash::{ActionHash, AgentPubKey, DhtOpHash, OpBasis};
use holochain_cascade::CascadeImpl;
use holochain_keystore::{AgentPubKeyExt, MetaLairClient};
use holochain_p2p::actor::GetActivityOptions;
use holochain_p2p::event::CountersigningSessionNegotiationMessage;
use holochain_p2p::{HolochainP2pDna, HolochainP2pDnaT};
use holochain_state::chain_lock::get_chain_lock;
use holochain_state::integrate::authored_ops_to_dht_db_without_check;
use holochain_state::mutations;
use holochain_state::prelude::*;
use kitsune_p2p_types::tx2::tx2_utils::Share;
use kitsune_p2p_types::KitsuneError;
use rusqlite::{named_params, Transaction};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;

mod accept;

pub(crate) use accept::accept_countersigning_request;

/// Countersigning workspace to hold session state.
#[derive(Clone, Default)]
pub struct CountersigningWorkspace {
    inner: Share<CountersigningWorkspaceInner>,
}

/// The inner state of a countersigning workspace.
#[derive(Default)]
struct CountersigningWorkspaceInner {
    sessions: HashMap<AgentPubKey, SessionState>,
}

#[derive(Debug)]
enum SessionState {
    Accepted(PreflightRequest),
    /// Multiple responses in the outer vec, sets of responses in the inner vec
    SignaturesCollected(Vec<Vec<SignedAction>>),
    /// The session is in an unknown state and needs to be resolved.
    ///
    /// In most cases, we do know how we got into this state, but we treat it as unknown because
    /// we want to always go through the same checks when leaving a countersigning session in any
    /// way that is not a successful completion.
    Unknown(Option<SessionResolutionSummary>),
}

/// Summary of the workflow's attempts to resolve the outcome a failed countersigning session.
///
/// This tracks the numbers of attempts and the outcome of the most recent attempt.
#[derive(Debug)]
struct SessionResolutionSummary {
    /// How many attempts have been made to resolve the session.
    ///
    /// Attempts are made according to the frequency specified by [RETRY_UNKNOWN_SESSION_STATE_DELAY].
    ///
    /// This count is only correct for the current run of the Holochain conductor. If the conductor
    /// is restarted then this counter is also reset.
    pub attempts: usize,
    /// The time of the last attempt to resolve the session.
    pub last_attempt_at: Timestamp,
    /// The outcome of the most recent attempt to resolve the session.
    pub outcomes: Vec<SessionResolutionOutcome>,
}

/// The outcome for a single agent who participated in a countersigning session.
///
/// [NUM_AUTHORITIES_TO_QUERY] authorities are made to agent activity authorities for each agent,
/// and the decisions are collected into [SessionResolutionOutcome::decisions].
#[derive(Debug)]
struct SessionResolutionOutcome {
    /// The agent who participated in the countersigning session and is the subject of this
    /// resolution outcome.
    pub agent: AgentPubKey,
    /// The resolved decision for each authority for the subject agent.
    pub decisions: Vec<SessionCompletionDecision>,
}

const NUM_AUTHORITIES_TO_QUERY: usize = 3;
const RETRY_UNKNOWN_SESSION_STATE_DELAY: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, PartialEq)]
enum SessionCompletionDecision {
    Complete,
    Abandoned,
    Indeterminate,
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn countersigning_workflow(
    space: Space,
    network: Arc<impl HolochainP2pDnaT>,
    cell_id: CellId,
    conductor: ConductorHandle,
    self_trigger: TriggerSender,
    sys_validation_trigger: TriggerSender,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> WorkflowResult<WorkComplete> {
    tracing::debug!(
        "Starting countersigning workflow, with {} sessions",
        space
            .countersigning_workspace
            .inner
            .share_ref(|inner| Ok(inner.sessions.len()))
            .unwrap()
    );

    let signal_tx = conductor
        .get_signal_tx(&cell_id)
        .await
        .map_err(WorkflowError::other)?;

    refresh_workspace_state(&space, cell_id.clone(), signal_tx.clone());

    // Apply timeouts by moving accepted sessions to unknown state if their end time is in the past.
    space
        .countersigning_workspace
        .inner
        .share_mut(|inner, _| {
            inner
                .sessions
                .iter_mut()
                .for_each(|(author, session_state)| {
                    if let SessionState::Accepted(request) = session_state {
                        if request.session_times.end < Timestamp::now() {
                            *session_state = SessionState::Unknown(None);
                        }
                    }
                });

            Ok(())
        })
        .unwrap();

    // Find sessions that are in an unknown state, we need to try to resolve those.
    let sessions_in_unknown_state = space
        .countersigning_workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .sessions
                .iter()
                .filter_map(|(author, session_state)| match session_state {
                    SessionState::Unknown(_) => Some(author.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>())
        })
        .unwrap();

    let mut remaining_sessions_in_unknown_state = 0;
    for author in sessions_in_unknown_state {
        tracing::info!(
            "Countersigning session for agent {:?} is in an unknown state, attempting to resolve",
            author
        );
        match inner_countersigning_session_incomplete(
            space.clone(),
            network.clone(),
            author.clone(),
            integration_trigger.clone(),
        )
        .await
        {
            Ok(
                decision @ (SessionCompletionDecision::Abandoned, _)
                | decision @ (SessionCompletionDecision::Complete, _),
            ) => {
                // The session state has been resolved, so we can remove it from the workspace.
                space
                    .countersigning_workspace
                    .inner
                    .share_mut(|inner, _| {
                        inner.sessions.remove(&author);
                        Ok(())
                    })
                    .unwrap();

                match decision {
                    (SessionCompletionDecision::Abandoned, _) => {
                        signal_tx
                            .send(Signal::System(SystemSignal::AbandonedCountersigning(
                                cell_id.clone(),
                            )))
                            .ok();
                    }
                    (SessionCompletionDecision::Complete, Some(cs_entry_hash)) => {
                        signal_tx
                            .send(Signal::System(SystemSignal::SuccessfulCountersigning(
                                cs_entry_hash,
                            )))
                            .ok();
                    }
                    (SessionCompletionDecision::Complete, None) => {
                        tracing::error!("Countersigning session completed but no entry hash was returned, this is a bug");
                    }
                    _ => unreachable!(),
                }
            }
            Ok((SessionCompletionDecision::Indeterminate, _)) => {
                remaining_sessions_in_unknown_state += 1;
                tracing::info!(
                    "Failed to cleanup countersigning session for agent: {:?}",
                    author
                );
            }
            Err(e) => {
                tracing::error!(
                    "Error cleaning up countersigning session for agent: {:?}: {:?}",
                    author,
                    e
                );
            }
        }
    }

    if remaining_sessions_in_unknown_state > 0 {
        tokio::task::spawn({
            let self_trigger = self_trigger.clone();
            async move {
                tokio::time::sleep(RETRY_UNKNOWN_SESSION_STATE_DELAY).await;
                self_trigger.trigger(&"unknown_session_state_retry");
            }
        });
    }

    let completed_sessions = space
        .countersigning_workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .sessions
                .iter()
                .filter_map(|(author, session_state)| match session_state {
                    SessionState::SignaturesCollected(signatures) => {
                        Some((author.clone(), signatures.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>())
        })
        .unwrap();

    for (author, signatures) in completed_sessions {
        for signature_bundle in signatures {
            // Try to complete the session using this signature bundle.

            match inner_countersigning_session_complete(
                space.clone(),
                network.clone(),
                author.clone(),
                signature_bundle.clone(),
                integration_trigger.clone(),
                publish_trigger.clone(),
            )
            .await
            {
                Ok(Some(cs_entry_hash)) => {
                    // If the session completed successfully with this bundle then we can remove the
                    // session from the workspace.
                    space
                        .countersigning_workspace
                        .inner
                        .share_mut(|inner, _| {
                            inner.sessions.remove(&author);
                            Ok(())
                        })
                        .unwrap();

                    // Signal to the UI.
                    // If there are no active connections this won't emit anything.
                    signal_tx
                        .send(Signal::System(SystemSignal::SuccessfulCountersigning(
                            cs_entry_hash,
                        )))
                        .ok();

                    break;
                }
                Ok(None) => {
                    tracing::warn!("Rejected signature bundle for countersigning session for agent: {:?}: {:?}", author, signature_bundle);
                }
                Err(e) => {
                    tracing::error!(
                        "Error completing countersigning session for agent: {:?}: {:?}",
                        author,
                        e
                    );
                }
            }
        }
    }

    // At the end of the workflow, if we have any sessions still in progress, then schedule a
    // workflow run for the one that will finish soonest.
    let maybe_earliest_finish = space
        .countersigning_workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .sessions
                .values()
                .filter_map(|s| {
                    match s {
                        SessionState::Accepted(request) => Some(request.session_times.end),
                        state => {
                            // Could be waiting for more signatures, or in an unknown state.
                            None
                        }
                    }
                })
                .min())
        })
        .unwrap();

    if let Some(earliest_finish) = maybe_earliest_finish {
        let delay = match (earliest_finish - Timestamp::now()).map(|d| d.to_std()) {
            Ok(Ok(d)) => d,
            _ => Duration::from_millis(100),
        };
        tracing::debug!("Countersigning workflow will run again in {:?}", delay);
        tokio::task::spawn(async move {
            tokio::time::sleep(delay).await;
            self_trigger.trigger(&"retrigger_expiry_check");
        });
    }

    Ok(WorkComplete::Complete)
}

async fn refresh_workspace_state(
    space: &Space,
    cell_id: CellId,
    signal: broadcast::Sender<Signal>,
) {
    let workspace = &space.countersigning_workspace;

    // We don't want to keep the entire space locked for writes, so just get the agents and release the lock.
    // We can then lock each agent individually.
    let agents = {
        space
            .authored_dbs
            .lock()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };

    let mut locked_for_agents = HashSet::new();
    for agent in agents {
        if let Ok(authored_db) = space.get_or_create_authored_db(agent.clone()) {
            let lock = authored_db
                .read_async({
                    let agent = agent.clone();
                    move |txn| get_chain_lock(&txn, &agent)
                })
                .await
                .ok()
                .flatten();

            // If the chain is locked, then we need to check the session state.
            if let Some(lock) = lock {
                locked_for_agents.insert(agent.clone());

                // Any locked chains, that aren't registered in the workspace, need to be added.
                // They have to go in as `Unknown` because we don't know the state of the session.
                workspace
                    .inner
                    .share_mut(|inner, _| {
                        inner
                            .sessions
                            .entry(agent.clone())
                            .or_insert(SessionState::Unknown(None));

                        Ok(())
                    })
                    .unwrap();
            }
        }
    }

    // Any sessions that are in the workspace but not locked need to be removed.
    let dropped_sessions = workspace
        .inner
        .share_mut(|inner, _| {
            let (keep, drop) = inner
                .sessions
                .drain()
                .partition::<HashMap<_, _>, _>(|(agent, _)| locked_for_agents.contains(agent));
            inner.sessions = keep;
            Ok(drop)
        })
        .unwrap();

    // This is expected to happen when the countersigning commit fails validation. The chain gets
    // unlocked, and we are just cleaning up state here.
    for (agent, _) in dropped_sessions {
        tracing::debug!("Countersigning session for agent {:?} is in the workspace but the chain is not locked, removing from workspace", agent);

        // Best effort attempt to let clients know that the session has been abandoned.
        signal
            .send(Signal::System(SystemSignal::AbandonedCountersigning(
                cell_id.clone(),
            )))
            .ok();
    }
}

/// Resolve an incomplete countersigning session.
///
/// This function is responsible for deciding what action to take when a countersigning session
/// has failed to complete.
///
/// The function returns true if the session state has been cleaned up and the chain has been unlocked.
/// Otherwise, the function returns false and the cleanup needs to be retried to resolved manually.
async fn inner_countersigning_session_incomplete(
    space: Space,
    network: Arc<impl HolochainP2pDnaT>,
    author: AgentPubKey,
    integration_trigger: TriggerSender,
) -> WorkflowResult<(SessionCompletionDecision, Option<EntryHash>)> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;

    let maybe_current_session =
        authored_db
            .write_async({
                let author = author.clone();
                move |txn| -> SourceChainResult<
                    Option<(Action, EntryHash, CounterSigningSessionData)>,
                > {
                    let maybe_current_session =
                        current_countersigning_session(txn, Arc::new(author.clone()))?;

                    // This is the simplest failure case, something has gone wrong but the countersigning entry
                    // hasn't been committed then we can unlock the chain and remove the session.
                    if maybe_current_session.is_none() {
                        mutations::unlock_chain(txn, &author)?;
                        return Ok(None);
                    }

                    Ok(maybe_current_session)
                }
            })
            .await?;

    if maybe_current_session.is_none() {
        tracing::info!("Countersigning session was in an unknown state but no session entry was found, unlocking chain and removing session: {:?}", author);
        return Ok((SessionCompletionDecision::Abandoned, None));
    }

    // Now things get more complicated. We have a countersigning entry on our chain but the session
    // is in a bad state. We need to figure out what the session state is and how to resolve it.

    let (cs_action, cs_entry_hash, session_data) = maybe_current_session.unwrap();

    // We need to find out what state the other signing agents are in.
    // TODO Note that we are ignoring the optional signing agents here - that's something we can figure out later because it's not clear what it means for them
    //      to be optional.
    let other_signing_agents = session_data
        .signing_agents()
        .filter(|a| **a != author)
        .collect::<Vec<_>>();

    let cascade = CascadeImpl::empty().with_network(network, space.cache_db.clone());

    let mut get_activity_options = holochain_p2p::actor::GetActivityOptions {
        include_warrants: true,
        include_full_actions: true,
        include_valid_activity: true,
        // We're going to be potentially running quite a lot of these requests, so set the timeout reasonably low.
        timeout_ms: Some(10_000),
        ..Default::default()
    };

    let mut by_agent_decisions = Vec::new();

    for agent in other_signing_agents {
        let agent_state = session_data.agent_state_for_agent(agent)?;

        // Query for the other agent's activity.
        // We only need a small sample to determine whether they've committed the session entry
        // or something else in the sequence number after their declared chain top.
        let filter = ChainQueryFilter::new()
            .sequence_range(ChainQueryFilterRange::ActionSeqRange(
                *agent_state.action_seq() + 1,
                agent_state.action_seq() + 1,
            ))
            .include_entries(true);

        let mut authority_decisions = Vec::new();
        let mut unusable_authority_responses = 0;

        // Try multiple times to get the activity for this agent.
        // Ideally, each request will go to a different authority, so we don't have to trust a single
        // authority. However, that is not guaranteed.
        for i in 0..NUM_AUTHORITIES_TO_QUERY {
            let activity_result = cascade
                .get_agent_activity(agent.clone(), filter.clone(), get_activity_options.clone())
                .await;

            let decision = match activity_result {
                Ok(activity) => {
                    if !activity.warrants.is_empty() {
                        // If this agent is warranted then we can't make the decision on our agent's behalf
                        // whether to trust this agent or not.
                        tracing::info!(
                            "Ignoring evidence from agent {:?} because they are warranted",
                            agent
                        );
                        break;
                    }

                    match activity.valid_activity {
                        ChainItems::Full(ref items) => {
                            println!("items: {:?}", items.len());

                            if items.len() == 0 {
                                // The agent has not published any new activity
                                SessionCompletionDecision::Abandoned
                            } else if items.len() > 1 {
                                tracing::warn!("Agent authority returned an unexpected response for agent {:?}: {:?}", agent, activity);
                                // Continue to try a different authority.
                                unusable_authority_responses += 1;
                                continue;
                            } else {
                                let maybe_countersigning_record = &items[0];
                                match &maybe_countersigning_record.entry {
                                    RecordEntry::Present(Entry::CounterSign(cs, _)) => {
                                        // Check that this is the same session, and not some other session on the other agent's chain.
                                        if cs.preflight_request == session_data.preflight_request {
                                            // This is evidence that the other agent has put the countersigning entry on their chain.
                                            // That is a strong signal that the session is complete.
                                            SessionCompletionDecision::Complete
                                        } else {
                                            // This is evidence that the other agent has put something else on their chain.
                                            SessionCompletionDecision::Abandoned
                                        }
                                    }
                                    RecordEntry::Present(_) => {
                                        // Anything else on the chain is evidence that the agent did not complete the countersigning.
                                        SessionCompletionDecision::Abandoned
                                    }
                                    RecordEntry::Hidden | RecordEntry::NA => {
                                        // This wouldn't be the case for a countersigning entry so this is evidence that
                                        // the agent has put something else on their chain.
                                        SessionCompletionDecision::Abandoned
                                    }
                                    RecordEntry::NotStored => {
                                        // This case is not determinate. The agent activity authority might
                                        // have the action but not the entry yet. We can't make a decision here.
                                        SessionCompletionDecision::Indeterminate
                                    }
                                }
                            }
                        }
                        _ => {
                            tracing::warn!("Agent authority returned an unexpected response for agent {:?}: {:?}", agent, activity);
                            unusable_authority_responses += 1;
                            continue;
                        }
                    }
                }
                e => {
                    tracing::debug!(
                        "Failed to get activity for agent {:?} because of: {:?}",
                        agent,
                        e
                    );
                    SessionCompletionDecision::Indeterminate
                }
            };

            authority_decisions.push(decision);
        }

        if unusable_authority_responses > 1 {
            // If more than one authority returned an unusable response then we can't make a decision.
            // We need to wait for more evidence.
            tracing::info!(
                "More than one authority returned an unusable response for agent {:?}, looking for more evidence",
                agent
            );
            continue;
        }

        if authority_decisions
            .iter()
            .all(|d| *d == SessionCompletionDecision::Complete)
        {
            by_agent_decisions.push(SessionCompletionDecision::Complete);
        } else if authority_decisions
            .iter()
            .all(|d| *d == SessionCompletionDecision::Abandoned)
        {
            by_agent_decisions.push(SessionCompletionDecision::Abandoned);
        } else {
            by_agent_decisions.push(SessionCompletionDecision::Indeterminate);
        }
    }

    let complete_decision_count = by_agent_decisions
        .iter()
        .filter(|d| **d == SessionCompletionDecision::Complete)
        .count();
    let abandoned_decision_count = by_agent_decisions
        .iter()
        .filter(|d| **d == SessionCompletionDecision::Abandoned)
        .count();

    // If there is no evidence that the session is complete, and we have evidence that the session
    // has been abandoned by a majority of agents, then we can make a decision and abandon the session
    if complete_decision_count == 0
        && abandoned_decision_count
            > (session_data.preflight_request.signing_agents.len() as f64 * 0.5).floor() as usize
    {
        abandon_session(authored_db, author.clone(), cs_action, cs_entry_hash).await?;

        // We can remove the session from the workspace and signal clients that the session is abandoned.
        return Ok((SessionCompletionDecision::Abandoned, None));
    }
    // If there is no evidence that the session is abandoned, and we have evidence that the session
    // has been completed by a majority of agents, then we can make a decision and complete the session
    else if abandoned_decision_count == 0
        && complete_decision_count
            > (session_data.preflight_request.signing_agents.len() as f64 * 0.5).floor() as usize
    {
        // Force local state changes and publish.
        //
        // Note: that this means we aren't doing signature verification or integrity checks here.
        // We also aren't able to participate in the publishing of signed actions to other agents.
        //
        // This should be okay because the point of gathering and verifying signatures is to check
        // that everyone involved in the session agreed. If we have found a sample of agents that
        // have gone beyond that step and publish their entries that is an alternative statement
        // of agreement.
        apply_success_state_changes(
            space,
            &session_data,
            &author,
            cs_action.to_hash(),
            integration_trigger,
        )
        .await?;

        return Ok((SessionCompletionDecision::Complete, Some(cs_entry_hash)));
    } else {
        tracing::info!(
            "Countersigning session state for agent {:?} is indeterminate based on evidence: {:?}",
            author,
            by_agent_decisions
        );
    }

    // We couldn't resolve the session state, try again later
    tracing::debug!(
        "Countersigning session state for agent {:?} is indeterminate, will retry later",
        author
    );
    Ok((SessionCompletionDecision::Indeterminate, None))
}

/// An incoming countersigning session success.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub(crate) async fn countersigning_success(
    space: Space,
    author: AgentPubKey,
    signed_actions: Vec<SignedAction>,
    countersigning_trigger: TriggerSender,
) {
    let should_trigger = space
        .countersigning_workspace
        .inner
        .share_mut(|inner, _| {
            match inner.sessions.entry(author.clone()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    match entry.get_mut() {
                        SessionState::Accepted(_) => {
                            entry.insert(SessionState::SignaturesCollected(vec![signed_actions]));
                        }
                        SessionState::SignaturesCollected(sigs) => {
                            sigs.push(signed_actions);
                        }
                        // TODO Could we handle this state transition by checking if the signatures
                        //      are valid for the unresolved session? That would allow signatures
                        //      to be received late. We don't really want that happening but if it
                        //      solves a problem for the current agent then that is a good thing.
                        SessionState::Unknown(_) => {
                            // If the session is in a bad state when we receive signatures then
                            // we can't do anything with them.
                            tracing::warn!("Countersigning session success received but the the session is in an unknown state for agent: {:?}", author);
                            return Ok(false);
                        }
                    }
                }
                std::collections::hash_map::Entry::Vacant(_) => {
                    // This will happen if the session has already been resolved and removed from
                    // internal state. Or if the conductor has restarted.
                    tracing::trace!("Countersigning session signatures received but is not in the workspace for agent: {:?}", author);
                    return Ok(false);
                }
            }

            Ok(true)
        })
        // Unwrap the error, because this share_mut callback doesn't return an error.
        .unwrap();

    if should_trigger {
        tracing::debug!("Received a signature bundle, triggering countersigning workflow");
        countersigning_trigger.trigger(&"countersigning_success");
    }
}

pub(crate) async fn inner_countersigning_session_complete(
    space: Space,
    network: Arc<impl HolochainP2pDnaT>,
    author: AgentPubKey,
    signed_actions: Vec<SignedAction>,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> WorkflowResult<Option<EntryHash>> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;
    let dht_db = space.dht_db.clone();
    let dht_db_cache = space.dht_query_cache.clone();

    // Using iterators is fine in this function as there can only be a maximum of 8 actions.
    let (this_cells_action_hash, entry_hash) = match signed_actions
        .iter()
        .find(|a| *a.author() == author)
        .and_then(|sa| {
            sa.entry_hash()
                .cloned()
                .map(|eh| (ActionHash::with_data_sync(sa), eh))
        }) {
        Some(a) => a,
        None => return Ok(None),
    };

    // Do a quick check to see if this entry hash matches the current locked session, so we don't
    // check signatures unless there is an active session.
    let reader_closure = {
        let entry_hash = entry_hash.clone();
        let author = author.clone();
        move |txn: Transaction| {
            // This chain lock isn't necessarily for the current session, we can't check that until later.
            if let Some((_, cs_entry_hash, session_data)) =
                current_countersigning_session(&txn, Arc::new(author.clone()))?
            {
                let lock_subject = holo_hash::encode::blake2b_256(
                    &holochain_serialized_bytes::encode(&session_data.preflight_request())?,
                );

                let chain_lock = holochain_state::chain_lock::get_chain_lock(&txn, &author)?;
                if let Some(chain_lock) = chain_lock {
                    // This is the case where we have already locked the chain for another session and are
                    // receiving another signature bundle from a different session. We don't need this, so
                    // it's safe to short circuit.
                    if cs_entry_hash != entry_hash || chain_lock.subject() != lock_subject {
                        return SourceChainResult::Ok(None);
                    }

                    let transaction: holochain_state::prelude::Txn = (&txn).into();
                    if transaction.contains_entry(&entry_hash)? {
                        return Ok(Some(session_data));
                    }
                }
            }
            SourceChainResult::Ok(None)
        }
    };

    let session_data = match authored_db.read_async(reader_closure).await? {
        Some(cs) => cs,
        None => {
            // If there is no active session then we can short circuit.
            tracing::warn!("Received a signature bundle for a session that exists in state but is missing from the database");
            return Ok(None);
        }
    };

    // Verify signatures of actions.
    let mut i_am_an_author = false;
    for sa in &signed_actions {
        if !sa
            .action()
            .author()
            .verify_signature(sa.signature(), sa.action())
            .await?
        {
            return Ok(None);
        }
        if sa.action().author() == &author {
            i_am_an_author = true;
        }
    }

    // Countersigning success is ultimately between authors to agree and publish.
    if !i_am_an_author {
        // We're effectively rejecting this signature bundle but communicating that this signature
        // bundle wasn't acceptable so that we can try another one.
        return Ok(None);
    }

    // Hash actions.
    let incoming_actions: Vec<_> = signed_actions
        .iter()
        .map(ActionHash::with_data_sync)
        .collect();

    let mut integrity_check_passed = false;

    let weight = weigh_placeholder();
    let stored_actions = session_data.build_action_set(entry_hash, weight)?;
    if stored_actions.len() == incoming_actions.len() {
        // Check all stored action hashes match an incoming action hash.
        if stored_actions.iter().all(|a| {
            let a = ActionHash::with_data_sync(a);
            incoming_actions.iter().any(|i| *i == a)
        }) {
            // All checks have passed, proceed to update the session state.
            integrity_check_passed = true;
        }
    }

    if !integrity_check_passed {
        // If the integrity check fails then we can't proceed with this signature bundle.
        return Ok(None);
    }

    apply_success_state_changes(
        space,
        &session_data,
        &author,
        this_cells_action_hash,
        integration_trigger,
    )
    .await?;

    // Publish other signers agent activity ops to their agent activity authorities.
    for sa in signed_actions {
        let (action, signature) = sa.into();
        if *action.author() == author {
            continue;
        }
        let op = ChainOp::RegisterAgentActivity(signature, action);
        let basis = op.dht_basis();
        if let Err(e) = network.publish_countersign(false, basis, op.into()).await {
            tracing::error!(
                "Failed to publish to other countersigners agent authorities because of: {:?}",
                e
            );
        }
    }

    publish_trigger.trigger(&"publish countersigning_success");

    tracing::info!("Countersigning session complete for agent: {:?}", author);

    Ok(Some(session_data.preflight_request.app_entry_hash))
}

async fn apply_success_state_changes(
    space: Space,
    session_data: &CounterSigningSessionData,
    author: &AgentPubKey,
    this_cells_action_hash: ActionHash,
    integration_trigger: TriggerSender,
) -> Result<(), WorkflowError> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;
    let dht_db = space.dht_db.clone();
    let dht_db_cache = space.dht_query_cache.clone();

    // Unlock the chain and remove the withhold publish flag from all ops in this session.
    let this_cell_actions_op_basis_hashes = authored_db
        .write_async({
            let author = author.clone();
            move |txn| -> SourceChainResult<Vec<DhtOpHash>> {
                // All checks have passed so unlock the chain.
                mutations::unlock_chain(txn, &author)?;
                // Update ops to publish.
                txn.execute(
                    "UPDATE DhtOp SET withhold_publish = NULL WHERE action_hash = :action_hash",
                    named_params! {
                    ":action_hash": this_cells_action_hash,
                    },
                )
                .map_err(holochain_state::prelude::StateMutationError::from)?;

                // Load the op hashes for this session so that we can publish them.
                Ok(get_countersigning_op_hashes(txn, this_cells_action_hash)?)
            }
        })
        .await?;

    // If all signatures are valid (above) and i signed then i must have
    // validated it previously so i now agree that i authored it.
    // TODO: perhaps this should be `authored_ops_to_dht_db`, i.e. the arc check should
    //       be performed, because we may not be an authority for these ops
    authored_ops_to_dht_db_without_check(
        this_cell_actions_op_basis_hashes,
        authored_db.into(),
        dht_db,
        &dht_db_cache,
    )
    .await?;

    integration_trigger.trigger(&"integrate countersigning_success");

    Ok(())
}

fn get_countersigning_op_hashes(
    txn: &mut Transaction,
    this_cells_action_hash: ActionHash,
) -> DatabaseResult<Vec<DhtOpHash>> {
    Ok(txn
        .prepare("SELECT basis_hash, hash FROM DhtOp WHERE action_hash = :action_hash")?
        .query_map(
            named_params! {
                ":action_hash": this_cells_action_hash
            },
            |row| {
                let hash: DhtOpHash = row.get("hash")?;
                Ok(hash)
            },
        )?
        .collect::<Result<Vec<_>, _>>()?)
}

/// Publish to entry authorities, so they can gather all the signed
/// actions for this session and respond with a session complete.
pub async fn countersigning_publish(
    network: &HolochainP2pDna,
    op: ChainOp,
    _author: AgentPubKey,
) -> Result<(), ZomeCallResponse> {
    if let Some(enzyme) = op.enzymatic_countersigning_enzyme() {
        if let Err(e) = network
            .countersigning_session_negotiation(
                vec![enzyme.clone()],
                CountersigningSessionNegotiationMessage::EnzymePush(Box::new(op)),
            )
            .await
        {
            tracing::error!(
                "Failed to push countersigning ops to enzyme because of: {:?}",
                e
            );
            return Err(ZomeCallResponse::CountersigningSession(e.to_string()));
        }
    } else {
        let basis = op.dht_basis();
        if let Err(e) = network.publish_countersign(true, basis, op.into()).await {
            tracing::error!(
                "Failed to publish to entry authorities for countersigning session because of: {:?}",
                e
            );
            return Err(ZomeCallResponse::CountersigningSession(e.to_string()));
        }
    }
    Ok(())
}

/// Abandon a countersigning session.
async fn abandon_session(
    authored_db: DbWrite<DbKindAuthored>,
    author: AgentPubKey,
    cs_action: Action,
    cs_entry_hash: EntryHash,
) -> StateMutationResult<()> {
    authored_db
        .write_async(move |txn| -> StateMutationResult<()> {
            // Do the dangerous thing and remove the countersigning session.
            remove_countersigning_session(txn, cs_action, cs_entry_hash)?;

            // Once the session is removed we can unlock the chain.
            unlock_chain(txn, &author)?;

            Ok(())
        })
        .await?;

    Ok(())
}
