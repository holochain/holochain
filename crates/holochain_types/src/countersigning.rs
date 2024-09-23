//! Types related to countersigning sessions.

use holo_hash::{AgentPubKey, EntryHash};
use holochain_zome_types::{
    cell::CellId,
    prelude::PreflightRequest,
    record::{SignedAction, SignedActionHashed},
};
use kitsune_p2p_dht::op::Timestamp;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// State and data of an ongoing countersigning session.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CountersigningSessionState {
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
        /// The preflight request that has been exchanged among countersigning peers.
        preflight_request: PreflightRequest,
        /// Signed actions of the committed countersigned entries of all participating peers.
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
        /// The preflight request that has been exchanged.
        preflight_request: PreflightRequest,
        /// Summary of the attempts to resolve this session.
        resolution: SessionResolutionSummary,
    },
}

impl CountersigningSessionState {
    /// Get preflight request of the countersigning session.
    pub fn preflight_request(&self) -> &PreflightRequest {
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

    /// Get app entry hash from preflight request.
    pub fn session_app_entry_hash(&self) -> &EntryHash {
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

/// Summary of the workflow's attempts to resolve the outcome a failed countersigning session.
///
/// This tracks the numbers of attempts and the outcome of the most recent attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResolutionSummary {
    /// The reason why session resolution is required.
    pub required_reason: ResolutionRequiredReason,
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

/// The reason why a countersigning session can not be resolved automatically and requires manual resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionRequiredReason {
    /// The session has timed out, so we should try to resolve its state before abandoning.
    Timeout,
    /// Something happened, like a conductor restart, and we lost track of the session.
    Unknown,
}

/// The outcome for a single agent who participated in a countersigning session.
///
/// [NUM_AUTHORITIES_TO_QUERY] authorities are made to agent activity authorities for each agent,
/// and the decisions are collected into [SessionResolutionOutcome::decisions].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResolutionOutcome {
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

/// Number of authorities to be queried for agent activity, in an attempt to resolve a countersigning
/// session in an unknown state.
pub const NUM_AUTHORITIES_TO_QUERY: usize = 3;

/// Decision about an incomplete countersigning session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SessionCompletionDecision {
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

/// Errors related to countersigning sessions.
#[derive(Debug, Error)]
pub enum CountersigningError {
    /// Countersigning workspace does not exist for cell.
    #[error("Countersigning workspace does not exist for cell id {0:?}")]
    WorkspaceDoesNotExist(CellId),
    /// No countersigning session found for the cell.
    #[error("No countersigning session found for cell id {0:?}")]
    SessionNotFound(CellId),
    /// Countersigning session in a resolvable state cannot be abandoned or published.
    #[error("Countersigning session for cell id {0:?} is resolvable. Only unresolvable sessions can be abandoned or published.")]
    SessionNotUnresolvable(CellId),
}
