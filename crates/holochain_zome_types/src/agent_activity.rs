use crate::{judged::Judged, HeaderType};
use crate::{EntryType, SignedHeader};
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GetAgentActivityInput {
    pub agent_pubkey: holo_hash::AgentPubKey,
    pub chain_query_filter: crate::query::ChainQueryFilter,
    pub activity_request: crate::query::ActivityRequest,
}

impl GetAgentActivityInput {
    /// Constructor.
    pub fn new(
        agent_pubkey: holo_hash::AgentPubKey,
        chain_query_filter: crate::query::ChainQueryFilter,
        activity_request: crate::query::ActivityRequest,
    ) -> Self {
        Self {
            agent_pubkey,
            chain_query_filter,
            activity_request,
        }
    }
}

/// Query arguments for the deterministic version of GetAgentActivity
#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq, Clone, Debug)]
pub struct DeterministicGetAgentActivityFilter {
    /// The upper and lower bound of headers to return.
    /// The lower bound is optional, and if omitted, will be set to the DNA element.
    pub range: (Option<HeaderHash>, HeaderHash),
    /// Filter by EntryType
    pub entry_type: Option<EntryType>,
    /// Filter by HeaderType
    pub header_type: Option<HeaderType>,
    /// Include the entries in the elements
    pub include_entries: bool,
}

#[derive(Debug)]
pub struct DeterministicGetAgentActivityResponse {
    pub chain: Vec<Judged<SignedHeader>>,
}

impl DeterministicGetAgentActivityResponse {
    pub fn new(chain: Vec<Judged<SignedHeader>>) -> Self {
        Self { chain }
    }
}
