//! Types for agents chain activity

use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// An agents chain records returned from a agent_activity_query
pub struct AgentActivityResponse {
    /// The agent this activity is for
    pub agent: AgentPubKey,
    /// Valid actions on this chain.
    pub valid_activity: ChainItems,
    /// Actions that were rejected by the agent activity
    /// authority and therefor invalidate the chain.
    pub rejected_activity: ChainItems,
    /// The status of this chain.
    pub status: ChainStatus,
    /// The highest chain action that has
    /// been observed by this authority.
    pub highest_observed: Option<HighestObserved>,
    /// Any warrants at the basis of this agent.
    pub warrants: Vec<Warrant>,
}

impl AgentActivityResponse {
    /// Convert an empty response to a different type.
    pub fn from_empty(other: AgentActivityResponse) -> Self {
        let convert_activity = |items: &ChainItems| match items {
            ChainItems::FullRecords(_) => ChainItems::FullRecords(Vec::with_capacity(0)),
            ChainItems::FullActions(_) => ChainItems::FullActions(Vec::with_capacity(0)),
            ChainItems::Hashes(_) => ChainItems::Hashes(Vec::with_capacity(0)),
            ChainItems::NotRequested => ChainItems::NotRequested,
        };
        AgentActivityResponse {
            agent: other.agent,
            valid_activity: convert_activity(&other.valid_activity),
            rejected_activity: convert_activity(&other.rejected_activity),
            status: ChainStatus::Empty,
            highest_observed: other.highest_observed,
            warrants: other.warrants,
        }
    }

    /// Convert to a status only response.
    pub fn status_only(other: AgentActivityResponse) -> Self {
        AgentActivityResponse {
            agent: other.agent,
            valid_activity: ChainItems::NotRequested,
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Empty,
            highest_observed: other.highest_observed,
            warrants: other.warrants,
        }
    }

    /// Convert to a [ChainItems::Hashes] response.
    pub fn hashes_only(other: AgentActivityResponse) -> Self {
        let convert_activity = |items: ChainItems| match items {
            ChainItems::FullRecords(records) => ChainItems::Hashes(
                records
                    .into_iter()
                    .map(|r| (r.action().action_seq(), r.address().clone()))
                    .collect(),
            ),
            ChainItems::FullActions(actions) => ChainItems::Hashes(
                actions
                    .into_iter()
                    .map(|a| (a.action().action_seq(), a.as_hash().clone()))
                    .collect(),
            ),
            ChainItems::Hashes(h) => ChainItems::Hashes(h),
            ChainItems::NotRequested => ChainItems::NotRequested,
        };
        AgentActivityResponse {
            agent: other.agent,
            valid_activity: convert_activity(other.valid_activity),
            rejected_activity: convert_activity(other.rejected_activity),
            status: other.status,
            highest_observed: other.highest_observed,
            warrants: other.warrants,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// The type of agent activity returned in this request
pub enum ChainItems {
    /// The full records
    FullRecords(Vec<Record>),
    /// The full actions
    FullActions(Vec<SignedActionHashed>),
    /// Just the hashes
    Hashes(Vec<(u32, ActionHash)>),
    /// Activity was not requested
    NotRequested,
}

impl From<AgentActivityResponse> for AgentActivity {
    fn from(a: AgentActivityResponse) -> Self {
        let valid_activity = match a.valid_activity {
            ChainItems::FullRecords(records) => records
                .into_iter()
                .map(|el| (el.action().action_seq(), el.action_address().clone()))
                .collect(),
            ChainItems::FullActions(actions) => actions
                .into_iter()
                .map(|el| (el.action().action_seq(), el.as_hash().clone()))
                .collect(),
            ChainItems::Hashes(h) => h,
            ChainItems::NotRequested => Vec::new(),
        };
        let rejected_activity = match a.rejected_activity {
            ChainItems::FullRecords(records) => records
                .into_iter()
                .map(|el| (el.action().action_seq(), el.action_address().clone()))
                .collect(),
            ChainItems::FullActions(actions) => actions
                .into_iter()
                .map(|el| (el.action().action_seq(), el.as_hash().clone()))
                .collect(),
            ChainItems::Hashes(h) => h,
            ChainItems::NotRequested => Vec::new(),
        };
        Self {
            valid_activity,
            rejected_activity,
            status: a.status,
            highest_observed: a.highest_observed,
            warrants: a.warrants,
        }
    }
}

/// A helper trait to allow [Record]s, [SignedActionHashed]s, and [ActionHashed]s to be converted into [ChainItems]
/// without needing to know which source type is being operated on.
pub trait ChainItemsSource {
    /// Convert a source type into a [ChainItems] value.
    fn to_chain_items(self) -> ChainItems;
}

impl ChainItemsSource for Vec<Record> {
    fn to_chain_items(self) -> ChainItems {
        ChainItems::FullRecords(self)
    }
}

impl ChainItemsSource for Vec<SignedActionHashed> {
    fn to_chain_items(self) -> ChainItems {
        ChainItems::FullActions(self)
    }
}

impl ChainItemsSource for Vec<ActionHashed> {
    fn to_chain_items(self) -> ChainItems {
        ChainItems::Hashes(
            self.into_iter()
                .map(|a| (a.action_seq(), a.as_hash().clone()))
                .collect(),
        )
    }
}

impl ChainItemsSource for Vec<(u32, ActionHash)> {
    fn to_chain_items(self) -> ChainItems {
        ChainItems::Hashes(self)
    }
}
