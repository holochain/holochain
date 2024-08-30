//! Countersigning workflow to maintain countersigning session state.

#![allow(unused)]

use super::error::WorkflowResult;
use crate::conductor::space::Space;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::{QueueTriggers, TriggerSender, WorkComplete};
use crate::core::ribosome::weigh_placeholder;
use crate::core::workflow::WorkflowError;
use either::Either;
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
use itertools::Itertools;
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
#[derive(Clone)]
pub struct CountersigningWorkspace {
    inner: Share<CountersigningWorkspaceInner>,
    countersigning_resolution_retry_delay: Duration,
}

impl CountersigningWorkspace {
    /// Create a new countersigning workspace.
    pub fn new(countersigning_resolution_retry_delay: Duration) -> Self {
        Self {
            inner: Default::default(),
            countersigning_resolution_retry_delay,
        }
    }
}

/// The inner state of a countersigning workspace.
#[derive(Default)]
struct CountersigningWorkspaceInner {
    sessions: HashMap<AgentPubKey, SessionState>,
}

#[derive(Debug)]
enum SessionState {
    /// This is the entry state. Accepting a countersigning session through the HDK will immediately
    /// register the countersigning session in this state, for management by the countersigning workflow.
    Accepted(PreflightRequest),
    /// This is the state where we have collected one or more signatures for a countersigning session.
    ///
    /// This state can be entered from the [SessionState::Accepted] state, which happens when a witness returns a
    /// signature bundle to us. While the session has not timed out, we will stay in this state and
    /// wait until one of the signatures bundles we have received is valid for the session to be
    /// completed.
    ///
    /// This state can also be entered from the [SessionState::Unknown] state, which happens when we
    /// have been able to recover the session from the source chain and have requested signed actions
    /// from agent authorities to build a signature bundle.
    ///
    /// From this state we either complete the session successfully, or we transition to the [SessionState::Unknown]
    /// state if we are unable to complete the session.
    SignaturesCollected {
        preflight_request: PreflightRequest,
        /// Multiple responses in the outer vec, sets of responses in the inner vec
        signature_bundles: Vec<Vec<SignedAction>>,
    },
    /// The session is in an unknown state and needs to be resolved.
    ///
    /// In most cases, we do know how we got into this state, but we treat it as unknown because
    /// we want to always go through the same checks when leaving a countersigning session in any
    /// way that is not a successful completion.
    ///
    /// Note that because the [PreflightRequest] is stored here, we only ever enter the unknown state
    /// if we managed to keep the preflight request in memory, or if we have been able to recover it
    /// from the source chain as part of the committed [CounterSigningSessionData]. Otherwise, we
    /// are unable to discover what session we were participating in, and we must abandon the session
    /// without going through this recovery state.
    Unknown {
        preflight_request: PreflightRequest,
        resolution: Option<SessionResolutionSummary>,
    },
}

impl SessionState {
    fn session_app_entry_hash(&self) -> &EntryHash {
        let request = match self {
            SessionState::Accepted(request) => request,
            SessionState::SignaturesCollected {
                preflight_request, ..
            } => preflight_request,
            SessionState::Unknown {
                preflight_request, ..
            } => preflight_request,
        };

        &request.app_entry_hash
    }
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

impl Default for SessionResolutionSummary {
    fn default() -> Self {
        Self {
            attempts: 0,
            last_attempt_at: Timestamp::now(),
            outcomes: Vec::new(),
        }
    }
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

#[derive(Clone, Debug, PartialEq)]
enum SessionCompletionDecision {
    Complete(Box<SignedActionHashed>),
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
                            *session_state = SessionState::Unknown {
                                preflight_request: request.clone(),
                                resolution: None,
                            };
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
                    SessionState::Unknown {
                        preflight_request: request,
                        ..
                    } => Some((author.clone(), request.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>())
        })
        .unwrap();

    let mut remaining_sessions_in_unknown_state = 0;
    for (author, preflight_request) in sessions_in_unknown_state {
        tracing::info!(
            "Countersigning session for agent {:?} is in an unknown state, attempting to resolve",
            author
        );
        match inner_countersigning_session_incomplete(
            space.clone(),
            network.clone(),
            author.clone(),
            integration_trigger.clone(),
            self_trigger.clone(),
        )
        .await
        {
            Ok(
                decision @ (SessionCompletionDecision::Abandoned, _)
                | decision @ (SessionCompletionDecision::Complete(_), _),
            ) => {
                // The session state has been resolved, so we can remove it from the workspace.
                let removed_session = space
                    .countersigning_workspace
                    .inner
                    .share_mut(|inner, _| {
                        let removed_session = inner.sessions.remove(&author);
                        Ok(removed_session)
                    })
                    .unwrap();

                if let Some(removed_session) = removed_session {
                    let entry_hash = removed_session.session_app_entry_hash().clone();

                    match decision {
                        (SessionCompletionDecision::Abandoned, _) => {
                            signal_tx
                                .send(Signal::System(SystemSignal::AbandonedCountersigning(
                                    entry_hash,
                                )))
                                .ok();
                        }
                        (SessionCompletionDecision::Complete(_), _) => {
                            signal_tx
                                .send(Signal::System(SystemSignal::SuccessfulCountersigning(
                                    entry_hash,
                                )))
                                .ok();
                        }
                        _ => unreachable!(),
                    }
                }
            }
            Ok((SessionCompletionDecision::Indeterminate, _)) => {
                remaining_sessions_in_unknown_state += 1;
                tracing::info!(
                    "No automated decision could be reached for the current countersigning session: {:?}",
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
                tokio::time::sleep(
                    space
                        .countersigning_workspace
                        .countersigning_resolution_retry_delay,
                )
                .await;
                self_trigger.trigger(&"unknown_session_state_retry");
            }
        });
    }

    let maybe_completed_sessions = space
        .countersigning_workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .sessions
                .iter()
                .filter_map(|(author, session_state)| match session_state {
                    SessionState::SignaturesCollected {
                        signature_bundles, ..
                    } => Some((author.clone(), signature_bundles.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>())
        })
        .unwrap();

    for (author, signatures) in maybe_completed_sessions {
        let mut found_valid_signature_bundle = false;
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

    // TODO This risks building up duplicate triggers
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
                // Try to retrieve the current countersigning session. If we can't then we have lost
                // the state of the session and need to unlock the chain.
                // This might happen if we were in the coordination phase of countersigning and the
                // conductor restarted.
                let maybe_current_session = authored_db
                    .write_async({
                        let agent = agent.clone();
                        move |txn| -> SourceChainResult<CurrentCountersigningSessionOpt> {
                            let maybe_current_session =
                                current_countersigning_session(txn, Arc::new(agent.clone()))?;
                            if maybe_current_session.is_none() {
                                unlock_chain(txn, &agent)?;
                            }
                            Ok(maybe_current_session)
                        }
                    })
                    .await
                    .ok()
                    .flatten();

                match maybe_current_session {
                    Some((_, _, session_data)) => {
                        locked_for_agents.insert(agent.clone());

                        // Any locked chains, that aren't registered in the workspace, need to be added.
                        // They have to go in as `Unknown` because we don't know the state of the session.
                        workspace
                            .inner
                            .share_mut(|inner, _| {
                                inner.sessions.entry(agent.clone()).or_insert(
                                    SessionState::Unknown {
                                        preflight_request: session_data.preflight_request().clone(),
                                        resolution: None,
                                    },
                                );

                                Ok(())
                            })
                            .unwrap();
                    }
                    None => {
                        // There was a stray chain lock but no countersigning session was found.
                        // The chain has been unlocked, so we don't include this agent in the `lock_for_agents` set.
                        // No further action needs to be taken, this agent can continue working on their chain.
                        tracing::info!("Found a stray chain lock for agent {:?} but no countersigning session was found. The chain has been unlocked", agent);
                    }
                }
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
    for (agent, session_state) in dropped_sessions {
        tracing::debug!("Countersigning session for agent {:?} is in the workspace but the chain is not locked, removing from workspace", agent);

        let entry_hash = session_state.session_app_entry_hash();

        // Best effort attempt to let clients know that the session has been abandoned.
        signal
            .send(Signal::System(SystemSignal::AbandonedCountersigning(
                entry_hash.clone(),
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
    countersigning_trigger: TriggerSender,
) -> WorkflowResult<(SessionCompletionDecision, Vec<SessionResolutionOutcome>)> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;

    let maybe_current_session = authored_db
        .write_async({
            let author = author.clone();
            move |txn| -> SourceChainResult<CurrentCountersigningSessionOpt> {
                let maybe_current_session =
                    current_countersigning_session(txn, Arc::new(author.clone()))?;

                // This is the simplest failure case, something has gone wrong but the countersigning entry
                // hasn't been committed then we can unlock the chain and remove the session.
                if maybe_current_session.is_none() {
                    unlock_chain(txn, &author)?;
                    return Ok(None);
                }

                Ok(maybe_current_session)
            }
        })
        .await?;

    if maybe_current_session.is_none() {
        tracing::info!("Countersigning session was in an unknown state but no session entry was found, unlocking chain and removing session: {:?}", author);
        return Ok((SessionCompletionDecision::Abandoned, Vec::with_capacity(0)));
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

    let mut get_activity_options = GetActivityOptions {
        include_warrants: true,
        include_valid_activity: true,
        include_full_records: true,
        get_options: GetOptions::network(),
        // We're going to be potentially running quite a lot of these requests, so set the timeout reasonably low.
        timeout_ms: Some(10_000),
        ..Default::default()
    };

    let mut by_agent_decisions = Vec::new();
    let mut resolution_outcomes = Vec::new();

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

        // Try multiple times to get the activity for this agent.
        // Ideally, each request will go to a different authority, so we don't have to trust a single
        // authority. However, that is not guaranteed.
        // TODO Should not need a loop here. The cascade/network should handle doing multiple
        //      requests. It's partially implemented but currently only sends to one authority.
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

                    match activity.status {
                        ChainStatus::Valid(ref head) => {
                            tracing::trace!("Agent {:?} has a valid chain: {:?}", agent, head);
                        }
                        status => {
                            tracing::info!(
                                "Agent {:?} has an invalid chain state for resolution: {:?}",
                                agent,
                                status
                            );
                            // Don't try to reason about this agent's state if the chain is invalid.
                            continue;
                        }
                    }

                    match activity.valid_activity {
                        ChainItems::Full(ref items) => {
                            if items.is_empty() {
                                // The agent has not published any new activity
                                SessionCompletionDecision::Abandoned
                            } else if items.len() > 1 {
                                tracing::warn!("Agent authority returned an unexpected response for agent {:?}: {:?}", agent, activity);
                                // Continue to try a different authority.
                                continue;
                            } else {
                                let maybe_countersigning_record = &items[0];
                                match &maybe_countersigning_record.entry {
                                    RecordEntry::Present(Entry::CounterSign(cs, _)) => {
                                        // Check that this is the same session, and not some other session on the other agent's chain.
                                        if cs.preflight_request == session_data.preflight_request {
                                            tracing::debug!("Agent {:?} has a countersigning entry for the same session", agent);
                                            // This agent appears to have completed the countersigning session.
                                            // Collect the signed action to use as a signature for completing the session for our agent.
                                            SessionCompletionDecision::Complete(Box::new(
                                                maybe_countersigning_record.signed_action.clone(),
                                            ))
                                        } else {
                                            // This is evidence that the other agent has put something else on their chain.
                                            // Specifically, some other countersigning session.
                                            SessionCompletionDecision::Abandoned
                                        }
                                    }
                                    RecordEntry::Present(_) => {
                                        // Anything else on the chain is evidence that the agent did not complete the countersigning.
                                        tracing::debug!(
                                            "Agent {:?} has a non-countersigning entry",
                                            agent
                                        );
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
                                        // In this case, we will also have tried to ask a record authority for
                                        // this missing entry.
                                        SessionCompletionDecision::Indeterminate
                                    }
                                }
                            }
                        }
                        _ => {
                            tracing::warn!("Agent authority returned an unexpected response for agent {:?}: {:?}", agent, activity);
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

        resolution_outcomes.push(SessionResolutionOutcome {
            agent: agent.clone(),
            decisions: authority_decisions.clone(),
        });

        if authority_decisions.len() < NUM_AUTHORITIES_TO_QUERY {
            // We are requiring all the authorities to agree, so if we don't have enough responses
            // then we can't make a decision.
            // That is likely to make the resolution process slower, but it's more likely to be correct.
            tracing::info!(
                "Not enough responses to make a decision for agent {:?}. Have {}/{}",
                agent,
                authority_decisions.len(),
                NUM_AUTHORITIES_TO_QUERY
            );
            by_agent_decisions.push(SessionCompletionDecision::Indeterminate);
            continue;
        }

        if authority_decisions
            .iter()
            .all(|d| matches!(*d, SessionCompletionDecision::Complete(_)))
        {
            // Safe to access without bounds check because we've done a size check above.
            tracing::debug!(
                "Authorities agree that agent {:?} has completed the session",
                agent
            );
            by_agent_decisions.push(authority_decisions[0].clone());
        } else if authority_decisions
            .iter()
            .all(|d| *d == SessionCompletionDecision::Abandoned)
        {
            tracing::debug!(
                "Authorities agree that agent {:?} has abandoned the session",
                agent
            );
            by_agent_decisions.push(SessionCompletionDecision::Abandoned);
        } else {
            // The decisions are either mixed or indeterminate. We can't make a decision so
            // collapse the responses to indeterminate.
            by_agent_decisions.push(SessionCompletionDecision::Indeterminate);
        }
    }

    let (mut signatures, abandoned): (Vec<SignedAction>, Vec<_>) = by_agent_decisions
        .into_iter()
        .filter(|d| *d != SessionCompletionDecision::Indeterminate)
        .partition_map(|d| match d {
            SessionCompletionDecision::Complete(sah) => Either::Left((*sah).into()),
            SessionCompletionDecision::Abandoned => Either::Right(()),
            _ => unreachable!(),
        });

    // Add our own action to the list of signatures.
    tracing::debug!(
        "Session resolution found {}/{} signatures and {}/{} abandoned",
        signatures.len(),
        session_data.preflight_request().signing_agents.len() - 1,
        abandoned.len(),
        session_data.preflight_request().signing_agents.len() - 1
    );

    signatures.push(cs_action.clone().into());

    if signatures.len() == session_data.preflight_request().signing_agents.len() {
        // We have all the signatures we need to complete the session. We can complete the session
        // without further action from our agent.
        // This is equivalent to receiving a signature bundle from a witness.
        tracing::debug!(
            "Submitting signature bundle to complete countersigning session for agent {:?}",
            author
        );
        countersigning_success(space, author.clone(), signatures, countersigning_trigger).await;

        // We haven't actually succeeded at this point, because the workflow will need to run again
        // to try and process the new signature bundle. We communicate completion here but the
        // caller must not change the session state based on this response.
        return Ok((
            SessionCompletionDecision::Complete(cs_action.into()),
            Vec::with_capacity(0),
        ));
    } else if abandoned.len() == session_data.preflight_request().signing_agents.len() - 1 {
        // We have evidence from all the authorities that we contacted, that all the other agents
        // in this session have abandoned the session. We can abandon the session too.
        // Note that for a two party session, this just means one other agent!
        tracing::debug!("All other agents have abandoned the countersigning session, abandoning session for agent {:?}", author);
        abandon_session(
            authored_db,
            author.clone(),
            cs_action.action().clone(),
            cs_entry_hash,
        )
        .await?;
        return Ok((SessionCompletionDecision::Abandoned, Vec::with_capacity(0)));
    }

    // Otherwise, we need to be cautious. Expose the current state of the session to the user so that
    // they can force a decision if they wish. However, Holochain cannot make a decision at this point
    // because we aren't absolutely sure that the session is complete or abandoned.

    tracing::debug!(
        "Countersigning session state for agent {:?} is indeterminate, will retry later",
        author
    );
    Ok((
        SessionCompletionDecision::Indeterminate,
        resolution_outcomes,
    ))
}

/// An incoming countersigning session success.
#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(space, signed_actions, countersigning_trigger))
)]
pub(crate) async fn countersigning_success(
    space: Space,
    author: AgentPubKey,
    signature_bundle: Vec<SignedAction>,
    countersigning_trigger: TriggerSender,
) {
    let should_trigger = space
        .countersigning_workspace
        .inner
        .share_mut(|inner, _| {
            match inner.sessions.entry(author.clone()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    match entry.get() {
                        // Whether we're awaiting signatures for the first time or trying to recover,
                        // switch to the signatures collected state and add the signatures to the
                        // list of signature bundles to try.
                        SessionState::Accepted(ref preflight_request) | SessionState::Unknown { ref preflight_request, .. } => {
                            entry.insert(SessionState::SignaturesCollected {
                                preflight_request: preflight_request.clone(),
                                signature_bundles: vec![signature_bundle],
                            });
                        }
                        SessionState::SignaturesCollected { preflight_request, signature_bundles} => {
                            entry.insert(SessionState::SignaturesCollected {
                                preflight_request: preflight_request.clone(),
                                signature_bundles: {
                                    let mut signature_bundles = signature_bundles.clone();
                                    signature_bundles.push(signature_bundle);
                                    signature_bundles
                                },
                            });
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
