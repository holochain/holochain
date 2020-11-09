//! Types for agents chain activity

use holo_hash::{AgentPubKey, HeaderHash};
use holochain_serialized_bytes::prelude::*;
pub use holochain_zome_types::query::ChainStatus;
use holochain_zome_types::{
    element::{Element, SignedHeaderHashed},
    query::HighestObserved,
};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// An agents chain elements returned from a agent_activity_query
pub struct AgentActivity<T = SignedHeaderHashed> {
    /// The agent this activity is for
    pub agent: AgentPubKey,
    /// Valid headers on this chain.
    pub valid_activity: ChainItems<T>,
    /// Headers that were rejected by the agent activity
    /// authority and therefor invalidate the chain.
    pub rejected_activity: ChainItems<T>,
    /// The status of this chain.
    pub status: ChainStatus,
    /// The highest chain header that has
    /// been observed by this authority.
    pub highest_observed: Option<HighestObserved>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// The type of agent activity returned in this request
pub enum ChainItems<T = SignedHeaderHashed> {
    /// The full headers
    Full(Vec<T>),
    /// Just the hashes
    Hashes(Vec<(u32, HeaderHash)>),
    /// Activity was not requested
    NotRequested,
}

impl From<AgentActivity<Element>> for holochain_zome_types::query::AgentActivity {
    fn from(a: AgentActivity<Element>) -> Self {
        let valid_activity = match a.valid_activity {
            ChainItems::Full(elements) => elements
                .into_iter()
                .map(|el| (el.header().header_seq(), el.header_address().clone()))
                .collect(),
            ChainItems::Hashes(h) => h,
            ChainItems::NotRequested => Vec::new(),
        };
        let rejected_activity = match a.rejected_activity {
            ChainItems::Full(elements) => elements
                .into_iter()
                .map(|el| (el.header().header_seq(), el.header_address().clone()))
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
