//! Types related to countersigning sessions.

use holo_hash::{AgentPubKey, EntryHash};
use holochain_zome_types::{
    prelude::PreflightRequest,
    record::{SignedAction, SignedActionHashed},
};
use kitsune_p2p_dht::op::Timestamp;
use serde::{Deserialize, Serialize};

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
    /// complete the session successfully or the session will time out and be abandoned.
    ///
    /// This state can also be entered from the [CountersigningSessionState::Unknown] state, which happens when we
    /// have been able to recover the session from the source chain and have requested signed actions
    /// from agent authorities to build a signature bundle.
    ///
    /// If we entered this state from the [CountersigningSessionState::Unknown] state, we will either
    /// complete the session successfully, or if the signatures are invalid, we will return to the
    /// [CountersigningSessionState::Unknown] state.
    SignaturesCollected {
        /// The preflight request that has been exchanged.
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
    /// Note that because the [PreflightRequest] is stored here, we only ever enter the unknown state
    /// if we managed to keep the preflight request in memory, or if we have been able to recover it
    /// from the source chain as part of the committed [CounterSigningSessionData]. Otherwise, we
    /// are unable to discover what session we were participating in, and we must abandon the session
    /// without going through this recovery state.
    Unknown {
        /// The preflight request that has been exchanged.
        preflight_request: PreflightRequest,
        /// Summary of the attempts to resolve this session.
        resolution: Option<SessionResolutionSummary>,
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
    /// If we've tried to complete the session using signatures found with agent activity authorities
    /// and still not succeeded, then the number of attempts is recorded here.
    pub completion_attempts: usize,
}

impl Default for SessionResolutionSummary {
    fn default() -> Self {
        Self {
            attempts: 1,
            last_attempt_at: Timestamp::now(),
            outcomes: Vec::with_capacity(0),
            completion_attempts: 0,
        }
    }
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
    /// Complete by an action.
    Complete(Box<SignedActionHashed>),
    /// Session is to be abandoned.
    Abandoned,
    /// No decision made.
    Indeterminate,
}
