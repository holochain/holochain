//! Countersigning workflow to maintain countersigning session state.

use super::error::WorkflowResult;
use crate::conductor::space::Space;
use crate::core::queue_consumer::{TriggerSender, WorkComplete};
use holo_hash::AgentPubKey;
use holochain_p2p::event::CountersigningSessionNegotiationMessage;
use holochain_p2p::{HolochainP2pDna, HolochainP2pDnaT};
use holochain_state::prelude::*;
use kitsune_p2p_types::tx_utils::Share;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::Sender;
use tokio::task::AbortHandle;

/// Accept handler for starting countersigning sessions.
mod accept;

/// Inner workflow for resolving an incomplete countersigning session.
mod incomplete;

/// Inner workflow for completing a countersigning session based on received signatures.
mod complete;

/// State integrity function to ensure that the database and the workspace are in sync.
mod refresh;

/// Success handler for receiving signature bundles from the network.
mod success;

#[cfg(test)]
mod tests;

pub(crate) use accept::accept_countersigning_request;
use holochain_keystore::MetaLairClient;
use holochain_state::chain_lock::get_chain_lock;
pub(crate) use success::countersigning_success;

/// Countersigning workspace to hold session state.
#[derive(Clone)]
pub struct CountersigningWorkspace {
    inner: Share<CountersigningWorkspaceInner>,
    countersigning_resolution_retry_delay: Duration,
    countersigning_resolution_retry_limit: Option<usize>,
}

impl CountersigningWorkspace {
    /// Create a new countersigning workspace.
    pub fn new(
        countersigning_resolution_retry_delay: Duration,
        countersigning_resolution_retry_limit: Option<usize>,
    ) -> Self {
        Self {
            inner: Default::default(),
            countersigning_resolution_retry_delay,
            countersigning_resolution_retry_limit,
        }
    }
}

/// The inner state of a countersigning workspace.
#[derive(Default)]
struct CountersigningWorkspaceInner {
    session: Option<CountersigningSessionState>,
    next_trigger: Option<NextTrigger>,
}

#[derive(Debug, Clone)]
enum CountersigningSessionState {
    /// This is the entry state. Accepting a countersigning session through the HDK will immediately
    /// register the countersigning session in this state, for management by the countersigning workflow.
    ///
    /// The session will stay in this state even when the agent commits their countersigning entry and only
    /// move to the next state when the first signature bundle is received.
    Accepted(PreflightRequest),
    /// This is the state where we have collected one or more signatures for a countersigning session.
    ///
    /// This state can be entered from the [CountersigningSessionState::Accepted] state, which happens
    /// when a witness returns a signature bundle to us. While the session has not timed out, we will
    /// stay in this state and wait until one of the signatures bundles we have received is valid for
    /// the session to be completed.
    ///
    /// If we entered this state from the [CountersigningSessionState::Accepted] state, we will either
    /// complete the session successfully or the session will time out. On a timeout we will move
    /// to the [CountersigningSessionState::Unknown] for a limited number of attempts to recover the session.
    ///
    /// This state can also be entered from the [CountersigningSessionState::Unknown] state, which happens when we
    /// have been able to recover the session from the source chain and have requested signed actions
    /// from agent authorities to build a signature bundle.
    ///
    /// If we entered this state from the [CountersigningSessionState::Unknown] state, we will either
    /// complete the session successfully, or if the signatures are invalid, we will return to the
    /// [CountersigningSessionState::Unknown] state.
    SignaturesCollected {
        preflight_request: PreflightRequest,
        /// Multiple responses in the outer vec, sets of responses in the inner vec
        signature_bundles: Vec<Vec<SignedAction>>,
        /// This field is set when the signature bundle came from querying agent activity authorities
        /// in the unknown state. If we started from that state, we should return to it if the
        /// signature bundle is invalid. Otherwise, stay in this state and wait for more signatures.
        resolution: Option<SessionResolutionSummary>,
    },
    /// The session is in an unknown state and needs to be resolved.
    ///
    /// This state is used when we have lost track of the countersigning session. This happens if
    /// we have got far enough to create the countersigning entry but have crashed or restarted
    /// before we could complete the session. In this case we need to try to discover what the other
    /// agent or agents involved in the session have done.
    ///
    /// This state is also entered temporarily when we have published a signature and then the
    /// session has timed out. To avoid deadlocking with two parties both waiting for each other to
    /// proceed, we cannot stay in this state indefinitely. We will make a limited number of attempts
    /// to recover and if we cannot, we will abandon the session.
    ///
    /// The only exception to the attempt limiting is if we are unable to reach agent activity authorities
    /// to progress resolving the session. In this case, the attempts are not counted towards the
    /// configured limit. This does not protect us against a network partition where we can only see
    /// a subset of the network, but it does protect us against Holochain forcing a decision while
    /// it is unable to reach any peers.
    ///
    /// Note that because the [PreflightRequest] is stored here, we only ever enter the unknown state
    /// if we managed to keep the preflight request in memory, or if we have been able to recover it
    /// from the source chain as part of the committed [CounterSigningSessionData]. Otherwise, we
    /// are unable to discover what session we were participating in, and we must abandon the session
    /// without going through this recovery state.
    Unknown {
        preflight_request: PreflightRequest,
        resolution: SessionResolutionSummary,
    },
}

impl CountersigningSessionState {
    fn preflight_request(&self) -> &PreflightRequest {
        match self {
            CountersigningSessionState::Accepted(preflight_request) => preflight_request,
            CountersigningSessionState::SignaturesCollected {
                preflight_request, ..
            } => preflight_request,
            CountersigningSessionState::Unknown {
                preflight_request, ..
            } => preflight_request,
        }
    }

    fn session_app_entry_hash(&self) -> &EntryHash {
        let request = match self {
            CountersigningSessionState::Accepted(request) => request,
            CountersigningSessionState::SignaturesCollected {
                preflight_request, ..
            } => preflight_request,
            CountersigningSessionState::Unknown {
                preflight_request, ..
            } => preflight_request,
        };

        &request.app_entry_hash
    }
}

#[derive(Debug, Clone)]
enum ResolutionRequiredReason {
    /// The session has timed out, so we should try to resolve its state before abandoning.
    Timeout,
    /// Something happened, like a conductor restart, and we lost track of the session.
    Unknown,
}

/// Summary of the workflow's attempts to resolve the outcome a failed countersigning session.
///
/// This tracks the numbers of attempts and the outcome of the most recent attempt.
#[derive(Debug, Clone)]
struct SessionResolutionSummary {
    /// The reason why session resolution is required.
    required_reason: ResolutionRequiredReason,
    /// How many attempts have been made to resolve the session.
    ///
    /// Attempts are made according to the frequency specified by [RETRY_UNKNOWN_SESSION_STATE_DELAY].
    ///
    /// This count is only correct for the current run of the Holochain conductor. If the conductor
    /// is restarted then this counter is also reset.
    pub attempts: usize,
    /// The time of the last attempt to resolve the session.
    pub last_attempt_at: Option<Timestamp>,
    /// The outcome of the most recent attempt to resolve the session.
    pub outcomes: Vec<SessionResolutionOutcome>,
}

impl Default for SessionResolutionSummary {
    fn default() -> Self {
        Self {
            required_reason: ResolutionRequiredReason::Unknown,
            attempts: 0,
            last_attempt_at: None,
            outcomes: Vec::with_capacity(0),
        }
    }
}

/// The outcome for a single agent who participated in a countersigning session.
///
/// [NUM_AUTHORITIES_TO_QUERY] authorities are made to agent activity authorities for each agent,
/// and the decisions are collected into [SessionResolutionOutcome::decisions].
#[derive(Debug, Clone)]
struct SessionResolutionOutcome {
    /// The agent who participated in the countersigning session and is the subject of this
    /// resolution outcome.
    // Unused until the next PR
    #[allow(dead_code)]
    pub agent: AgentPubKey,
    /// The resolved decision for each authority for the subject agent.
    // Unused until the next PR
    #[allow(dead_code)]
    pub decisions: Vec<SessionCompletionDecision>,
}

const NUM_AUTHORITIES_TO_QUERY: usize = 3;

#[derive(Clone, Debug, PartialEq)]
enum SessionCompletionDecision {
    /// Evidence found on the network that this session completed successfully.
    Complete(Box<SignedActionHashed>),
    /// Evidence found on the network that this session was abandoned and other agents have
    /// added to their chain without completing the session.
    Abandoned,
    /// No evidence, or inconclusive evidence, was found on the network. Holochain will not make an
    /// automatic decision until the evidence is conclusive.
    Indeterminate,
    /// There were errors encountered while trying to resolve the session. Errors such as network
    /// errors are treated differently to inconclusive evidence. We don't want to force a decision
    /// when we're offline, for example. In this case, the resolution must be retried later and this
    /// attempt should not be counted.
    Failed,
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn countersigning_workflow(
    space: Space,
    workspace: Arc<CountersigningWorkspace>,
    network: Arc<impl HolochainP2pDnaT>,
    keystore: MetaLairClient,
    cell_id: CellId,
    signal_tx: Sender<Signal>,
    self_trigger: TriggerSender,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> WorkflowResult<WorkComplete> {
    tracing::debug!(
        "Starting countersigning workflow, with a session? {}",
        workspace
            .inner
            .share_ref(|inner| Ok(inner.session.is_some()))
            .unwrap()
    );

    // Clear trigger, if we need another one, it will be created later.
    workspace
        .inner
        .share_mut(|inner, _| {
            if let Some(next_trigger) = &mut inner.next_trigger {
                next_trigger.trigger_task.abort();
            }
            inner.next_trigger = None;
            Ok(())
        })
        .unwrap();

    // Ensure the workspace state knows about anything in the database on startup.
    refresh::refresh_workspace_state(
        &space,
        workspace.clone(),
        cell_id.clone(),
        signal_tx.clone(),
    )
    .await;

    // Abandon any sessions that have timed out.
    apply_timeout(&space, workspace.clone(), &cell_id, signal_tx.clone()).await?;

    // If the session is in an unknown state, try to recover it.
    try_recover_failed_session(
        &space,
        workspace.clone(),
        network.clone(),
        &cell_id,
        &signal_tx,
    )
    .await?;

    let maybe_completed_session = workspace
        .inner
        .share_mut(|inner, _| {
            Ok(match &mut inner.session {
                Some(CountersigningSessionState::SignaturesCollected {
                    signature_bundles, ..
                }) => Some(std::mem::take(signature_bundles)),
                _ => None,
            })
        })
        .unwrap();

    if let Some(signature_bundles) = maybe_completed_session {
        let mut completed = false;

        for signature_bundle in signature_bundles {
            // Try to complete the session using this signature bundle.

            match complete::inner_countersigning_session_complete(
                space.clone(),
                network.clone(),
                keystore.clone(),
                cell_id.agent_pubkey().clone(),
                signature_bundle.clone(),
                integration_trigger.clone(),
                publish_trigger.clone(),
            )
            .await
            {
                Ok(Some(cs_entry_hash)) => {
                    // The session completed successfully this bundle, so we can remove the session
                    // from the workspace.
                    workspace
                        .inner
                        .share_mut(|inner, _| {
                            tracing::trace!("Countersigning session completed successfully, removing from the workspace for agent: {:?}", cell_id.agent_pubkey());
                            inner.session = None;
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

                    completed = true;
                    break;
                }
                Ok(None) => {
                    tracing::warn!("Rejected signature bundle for countersigning session for agent: {:?}: {:?}", cell_id.agent_pubkey(), signature_bundle);
                }
                Err(e) => {
                    tracing::error!(
                        "Error completing countersigning session for agent: {:?}: {:?}",
                        cell_id.agent_pubkey(),
                        e
                    );
                }
            }
        }

        if !completed {
            // If we got these signatures from a resolution attempt, then we need to return to the
            // unknown state now that we've tried the signatures, and they can't be used to resolve
            // the session.
            workspace.inner.share_mut(|inner, _| {
                if let Some(session) = &mut inner.session {
                    match session {
                        CountersigningSessionState::SignaturesCollected {
                            preflight_request,
                            resolution,
                            ..
                        } => {
                            if resolution.is_some() {
                                *session = CountersigningSessionState::Unknown {
                                    preflight_request: preflight_request.clone(),
                                    resolution: resolution.clone().unwrap_or_default(),
                                };
                            }
                        }
                        _ => {
                            tracing::error!("Countersigning session for agent {:?} was not in the expected state while trying to resolve it: {:?}", cell_id.agent_pubkey(), session);
                        }
                    }
                }
                Ok(())
            }).unwrap();
        }
    }

    // At the end of the workflow, if we have a session still in progress, then schedule a
    // workflow run again at the end time.
    let maybe_end_time = workspace
        .inner
        .share_ref(|inner| {
            Ok(match &inner.session {
                Some(state) => match state {
                    CountersigningSessionState::Accepted(preflight_request)
                    | CountersigningSessionState::SignaturesCollected {
                        preflight_request, ..
                    } => Some(preflight_request.session_times.end),
                    CountersigningSessionState::Unknown { .. } => {
                        (Timestamp::now() + workspace.countersigning_resolution_retry_delay).ok()
                    }
                },
                None => None,
            })
        })
        .unwrap();

    tracing::trace!("End time: {:?}", maybe_end_time);

    if let Some(end_time) = maybe_end_time {
        reschedule_self(workspace, self_trigger, end_time);
    }

    Ok(WorkComplete::Complete)
}

async fn try_recover_failed_session(
    space: &Space,
    workspace: Arc<CountersigningWorkspace>,
    network: Arc<impl HolochainP2pDnaT + Sized>,
    cell_id: &CellId,
    signal_tx: &Sender<Signal>,
) -> WorkflowResult<()> {
    let maybe_session_in_unknown_state = workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .session
                .as_ref()
                .and_then(|session_state| match session_state {
                    CountersigningSessionState::Unknown {
                        preflight_request, ..
                    } => Some(preflight_request.clone()),
                    _ => None,
                }))
        })
        .unwrap();

    if let Some(preflight_request) = maybe_session_in_unknown_state {
        tracing::info!(
            "Countersigning session for agent {:?} is in an unknown state, attempting to resolve",
            cell_id.agent_pubkey()
        );
        match incomplete::inner_countersigning_session_incomplete(
            space.clone(),
            network.clone(),
            cell_id.agent_pubkey().clone(),
            preflight_request.clone(),
        )
        .await
        {
            Ok((SessionCompletionDecision::Complete(_), outcomes)) => {
                // No need to do anything here. Signatures were found which may be able to complete
                // the session but the session isn't actually complete yet. We need to let the
                // workflow re-run and try those signatures.
                update_last_attempted(workspace.clone(), true, outcomes, cell_id);
            }
            Ok((SessionCompletionDecision::Abandoned, _)) => {
                // The session state has been resolved, so we can remove it from the workspace.
                workspace
                    .inner
                    .share_mut(|inner, _| {
                        tracing::trace!(
                            "Decision made for incomplete session, removing from workspace: {:?}",
                            cell_id.agent_pubkey()
                        );

                        inner.session = None;
                        Ok(())
                    })
                    .unwrap();

                signal_tx
                    .send(Signal::System(SystemSignal::AbandonedCountersigning(
                        preflight_request.app_entry_hash.clone(),
                    )))
                    .ok();
            }
            Ok((SessionCompletionDecision::Indeterminate, outcomes)) => {
                tracing::info!(
                    "No automated decision could be reached for the current countersigning session: {:?}",
                    cell_id.agent_pubkey()
                );

                // Record the attempt
                update_last_attempted(workspace.clone(), true, outcomes, cell_id);

                let resolution = get_resolution(workspace.clone());
                if let Some(SessionResolutionSummary {
                    required_reason: ResolutionRequiredReason::Timeout,
                    attempts,
                    ..
                }) = resolution
                {
                    let limit = workspace.countersigning_resolution_retry_limit.unwrap_or(0);

                    // If we have reached the limit of attempts, then abandon the session.
                    if workspace.countersigning_resolution_retry_limit.is_none()
                        || (limit > 0 && attempts >= limit)
                    {
                        tracing::info!("Reached the limit ({}) of attempts ({}) to resolve countersigning session for agent: {:?}", limit, attempts, cell_id.agent_pubkey());

                        force_abandon_session(
                            space.clone(),
                            cell_id.agent_pubkey(),
                            &preflight_request,
                        )
                        .await?;

                        // The session state has been resolved, so we can remove it from the workspace.
                        workspace
                            .inner
                            .share_mut(|inner, _| {
                                tracing::trace!(
                                    "Abandoning countersigning session for agent: {:?}",
                                    cell_id.agent_pubkey()
                                );

                                inner.session = None;
                                Ok(())
                            })
                            .unwrap();

                        signal_tx
                            .send(Signal::System(SystemSignal::AbandonedCountersigning(
                                preflight_request.app_entry_hash.clone(),
                            )))
                            .ok();
                    }
                }
            }
            Ok((SessionCompletionDecision::Failed, outcomes)) => {
                tracing::info!(
                    "Failed to resolve countersigning session for agent: {:?}",
                    cell_id.agent_pubkey()
                );

                // Record the attempt time, but not the attempt count.
                update_last_attempted(workspace.clone(), false, outcomes, cell_id);
            }
            Err(e) => {
                tracing::error!(
                    "Error resolving countersigning session for agent: {:?}: {:?}",
                    cell_id.agent_pubkey(),
                    e
                );
            }
        }
    }

    Ok(())
}

fn reschedule_self(
    workspace: Arc<CountersigningWorkspace>,
    self_trigger: TriggerSender,
    at_timestamp: Timestamp,
) {
    workspace
        .inner
        .share_mut(|inner, _| {
            if let Some(next_trigger) = &mut inner.next_trigger {
                next_trigger.replace_if_sooner(at_timestamp, self_trigger.clone());
            } else {
                inner.next_trigger = Some(NextTrigger::new(at_timestamp, self_trigger.clone()));
            }

            Ok(())
        })
        .unwrap();
}

fn update_last_attempted(
    workspace: Arc<CountersigningWorkspace>,
    add_to_attempts: bool,
    outcomes: Vec<SessionResolutionOutcome>,
    cell_id: &CellId,
) {
    workspace.inner.share_mut(|inner, _| {
        if let Some(session) = &mut inner.session {
            match session {
                CountersigningSessionState::SignaturesCollected { resolution, .. } => {
                    if let Some(resolution) = resolution {
                        if add_to_attempts {
                            resolution.attempts += 1;
                        }
                        resolution.last_attempt_at = Some(Timestamp::now());
                        resolution.outcomes = outcomes;
                    } else {
                        tracing::warn!("Countersigning session for agent {:?} is missing a resolution but we are trying to resolve it", cell_id.agent_pubkey());
                    }
                }
                CountersigningSessionState::Unknown { resolution, .. } => {
                    if add_to_attempts {
                        resolution.attempts += 1;
                    }
                    resolution.last_attempt_at = Some(Timestamp::now());
                    resolution.outcomes = outcomes;
                }
                state => {
                    tracing::error!("Countersigning session for agent {:?} was not in the expected state while trying to resolve it: {:?}", cell_id.agent_pubkey(), state);
                }
            }
        } else {
            tracing::error!("Countersigning session for agent {:?} was removed from the workspace while trying to resolve it", cell_id.agent_pubkey());
        }

        Ok(())
    }).unwrap()
}

fn get_resolution(workspace: Arc<CountersigningWorkspace>) -> Option<SessionResolutionSummary> {
    workspace
        .inner
        .share_ref(|inner| {
            Ok(match &inner.session {
                Some(CountersigningSessionState::SignaturesCollected { resolution, .. }) => {
                    resolution.clone()
                }
                Some(CountersigningSessionState::Unknown { resolution, .. }) => {
                    Some(resolution.clone())
                }
                _ => None,
            })
        })
        .unwrap()
}

async fn apply_timeout(
    space: &Space,
    workspace: Arc<CountersigningWorkspace>,
    cell_id: &CellId,
    signal_tx: Sender<Signal>,
) -> WorkflowResult<()> {
    let preflight_request = workspace
        .inner
        .share_ref(|inner| {
            Ok(inner
                .session
                .as_ref()
                .map(|session| session.preflight_request().clone()))
        })
        .unwrap();
    if preflight_request.is_none() {
        tracing::info!("Cannot check session timeout because there is no active session");
        return Ok(());
    }

    let authored = space.get_or_create_authored_db(cell_id.agent_pubkey().clone())?;

    let current_session = authored
        .read_async({
            let author = cell_id.agent_pubkey().clone();
            move |txn| current_countersigning_session(&txn, Arc::new(author))
        })
        .await?;

    let mut has_committed_session = false;
    if let Some((_, _, session_data)) = current_session {
        if session_data.preflight_request.fingerprint() == preflight_request.unwrap().fingerprint()
        {
            has_committed_session = true;
        }
    }

    let timed_out = workspace
        .inner
        .share_mut(|inner, _| {
            Ok(inner.session.as_mut().and_then(|session| {
                let expired = match session {
                    CountersigningSessionState::Accepted(preflight_request) => {
                        if preflight_request.session_times.end < Timestamp::now() {
                            if has_committed_session {
                                *session = CountersigningSessionState::Unknown {
                                    preflight_request: preflight_request.clone(),
                                    resolution: SessionResolutionSummary {
                                        required_reason: ResolutionRequiredReason::Timeout,
                                        ..Default::default()
                                    },
                                };
                                false
                            } else {
                                true
                            }
                        } else {
                            false
                        }
                    }
                    CountersigningSessionState::SignaturesCollected {
                        preflight_request,
                        signature_bundles,
                        resolution,
                    } => {
                        // Only change state if all signatures have been tried and this is not a recovery state
                        // because recovery should be dealt with separately.
                        if preflight_request.session_times.end < Timestamp::now()
                            && signature_bundles.is_empty()
                            && resolution.is_none()
                        {
                            *session = CountersigningSessionState::Unknown {
                                preflight_request: preflight_request.clone(),
                                resolution: SessionResolutionSummary {
                                    required_reason: ResolutionRequiredReason::Timeout,
                                    ..Default::default()
                                },
                            };
                        }

                        false
                    }
                    _ => false,
                };

                if expired {
                    Some(session.preflight_request().clone())
                } else {
                    None
                }
            }))
        })
        .unwrap();

    if let Some(preflight_request) = timed_out {
        tracing::info!(
            "Countersigning session for agent {:?} has timed out, abandoning session",
            cell_id.agent_pubkey()
        );

        match force_abandon_session(space.clone(), cell_id.agent_pubkey(), &preflight_request).await
        {
            Ok(_) => {
                // Only once we've managed to remove the session do we remove the state for it.
                workspace
                    .inner
                    .share_mut(|inner, _| {
                        inner.session = None;
                        Ok(())
                    })
                    .unwrap();

                // Then let the client know.
                signal_tx
                    .send(Signal::System(SystemSignal::AbandonedCountersigning(
                        preflight_request.app_entry_hash,
                    )))
                    .ok();
            }
            Err(e) => {
                tracing::error!(
                    "Error abandoning countersigning session for agent: {:?}: {:?}",
                    cell_id.agent_pubkey(),
                    e
                );
            }
        }
    }

    Ok(())
}

async fn force_abandon_session(
    space: Space,
    author: &AgentPubKey,
    preflight_request: &PreflightRequest,
) -> SourceChainResult<()> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;

    let abandon_fingerprint = preflight_request.fingerprint()?;

    let maybe_session_data = authored_db
        .write_async({
            let author = author.clone();
            move |txn| current_countersigning_session(txn, Arc::new(author.clone()))
        })
        .await?;

    match maybe_session_data {
        Some((cs_action, cs_entry_hash, x))
            if x.preflight_request.fingerprint()? == abandon_fingerprint =>
        {
            tracing::info!("There is a committed session to remove for: {:?}", author);
            abandon_session(
                authored_db,
                author.clone(),
                cs_action.action().clone(),
                cs_entry_hash,
            )
            .await?;
        }
        _ => {
            // There is no matching, committed session but there may be a lock to remove
            authored_db
                .write_async({
                    let author = author.clone();
                    move |txn| {
                        let chain_lock = get_chain_lock(txn, &author)?;

                        match chain_lock {
                            Some(lock) if lock.subject() == abandon_fingerprint => {
                                unlock_chain(txn, &author)
                            }
                            _ => {
                                tracing::warn!(
                                    "No matching session or lock to remove for: {:?}",
                                    author
                                );
                                Ok(())
                            }
                        }
                    }
                })
                .await?;
        }
    }

    Ok(())
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

// TODO unify with the other mechanisms for re-triggering. This is currently working around
//      a performance issue with WorkComplete::Incomplete but is similar to the loop logic that
//      other workflows use - the difference being that this workflow varies the loop delay.
struct NextTrigger {
    trigger_at: Timestamp,
    trigger_task: AbortHandle,
}

impl NextTrigger {
    fn new(trigger_at: Timestamp, trigger_sender: TriggerSender) -> Self {
        let delay = Self::calculate_delay(&trigger_at);

        let trigger_task = Self::start_trigger_task(delay, trigger_sender);

        Self {
            trigger_at,
            trigger_task,
        }
    }

    fn replace_if_sooner(&mut self, trigger_at: Timestamp, trigger_sender: TriggerSender) {
        // If the current trigger has expired, or the new one is sooner, then replace the
        // current trigger.
        if self.trigger_at < Timestamp::now() || trigger_at < self.trigger_at {
            let new_delay = Self::calculate_delay(&trigger_at);
            self.trigger_task.abort();
            self.trigger_at = trigger_at;
            self.trigger_task = Self::start_trigger_task(new_delay, trigger_sender);
        }
    }

    fn calculate_delay(trigger_at: &Timestamp) -> Duration {
        match trigger_at
            .checked_difference_signed(&Timestamp::now())
            .map(|d| d.to_std())
        {
            Some(Ok(d)) => d,
            _ => Duration::from_millis(100),
        }
    }

    fn start_trigger_task(delay: Duration, trigger_sender: TriggerSender) -> AbortHandle {
        tracing::trace!("Scheduling countersigning workflow in: {:?}", delay);
        tokio::task::spawn(async move {
            tokio::time::sleep(delay).await;
            trigger_sender.trigger(&"next trigger");
        })
        .abort_handle()
    }
}
