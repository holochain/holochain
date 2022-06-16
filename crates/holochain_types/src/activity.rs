//! Types for agents chain activity

use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// An agents chain elements returned from a agent_activity_query
pub struct AgentActivityResponse<T = SignedActionHashed> {
    /// The agent this activity is for
    pub agent: AgentPubKey,
    /// Valid actions on this chain.
    pub valid_activity: ChainItems<T>,
    /// Actions that were rejected by the agent activity
    /// authority and therefor invalidate the chain.
    pub rejected_activity: ChainItems<T>,
    /// The status of this chain.
    pub status: ChainStatus,
    /// The highest chain action that has
    /// been observed by this authority.
    pub highest_observed: Option<HighestObserved>,
}

holochain_serial!(AgentActivityResponse<ActionHash>);

impl<A> AgentActivityResponse<A> {
    /// Convert an empty response to a different type.
    pub fn from_empty<B>(other: AgentActivityResponse<B>) -> Self {
        let convert_activity = |items: &ChainItems<B>| match items {
            ChainItems::Full(_) => ChainItems::Full(Vec::with_capacity(0)),
            ChainItems::Hashes(_) => ChainItems::Hashes(Vec::with_capacity(0)),
            ChainItems::NotRequested => ChainItems::NotRequested,
        };
        AgentActivityResponse {
            agent: other.agent,
            valid_activity: convert_activity(&other.valid_activity),
            rejected_activity: convert_activity(&other.rejected_activity),
            status: ChainStatus::Empty,
            highest_observed: other.highest_observed,
        }
    }

    /// Convert an status only response to a different type.
    pub fn status_only<B>(other: AgentActivityResponse<B>) -> Self {
        AgentActivityResponse {
            agent: other.agent,
            valid_activity: ChainItems::NotRequested,
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Empty,
            highest_observed: other.highest_observed,
        }
    }

    /// Convert an hashes only response to a different type.
    pub fn hashes_only<B>(other: AgentActivityResponse<B>) -> Self {
        let convert_activity = |items: ChainItems<B>| match items {
            ChainItems::Full(_) => ChainItems::Full(Vec::with_capacity(0)),
            ChainItems::Hashes(h) => ChainItems::Hashes(h),
            ChainItems::NotRequested => ChainItems::NotRequested,
        };
        AgentActivityResponse {
            agent: other.agent,
            valid_activity: convert_activity(other.valid_activity),
            rejected_activity: convert_activity(other.rejected_activity),
            status: other.status,
            highest_observed: other.highest_observed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// The type of agent activity returned in this request
pub enum ChainItems<T = SignedActionHashed> {
    /// The full actions
    Full(Vec<T>),
    /// Just the hashes
    Hashes(Vec<(u32, ActionHash)>),
    /// Activity was not requested
    NotRequested,
}

impl From<AgentActivityResponse<Element>> for holochain_zome_types::query::AgentActivity {
    fn from(a: AgentActivityResponse<Element>) -> Self {
        let valid_activity = match a.valid_activity {
            ChainItems::Full(elements) => elements
                .into_iter()
                .map(|el| (el.action().action_seq(), el.action_address().clone()))
                .collect(),
            ChainItems::Hashes(h) => h,
            ChainItems::NotRequested => Vec::new(),
        };
        let rejected_activity = match a.rejected_activity {
            ChainItems::Full(elements) => elements
                .into_iter()
                .map(|el| (el.action().action_seq(), el.action_address().clone()))
                .collect(),
            ChainItems::Hashes(h) => h,
            ChainItems::NotRequested => Vec::new(),
        };
        Self {
            valid_activity,
            rejected_activity,
            status: a.status,
            highest_observed: a.highest_observed,
            warrants: Vec::with_capacity(0),
        }
    }
}
