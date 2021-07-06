use crate::prelude::*;
use holo_hash::AgentPubKey;
use holo_hash::HeaderHash;
use holo_hash::EntryHash;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterSigningSessionTimes{
    start: Timestamp,
    end: Timestamp,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct PreflightBytes(#[serde(with = "serde_bytes")] Vec<u8>);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Role(u8);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreflightRequest {
    signing_agents: Vec<(AgentPubKey, Vec<Role>)>,
    enzyme_index: Option<u8>,
    session_times: CounterSigningSessionTimes,
    header_base: HeaderBase,
    preflight_bytes: PreflightBytes,
}

pub struct PreflightResponse {
    request: PreflightRequest,
    agent_index: u8,
    agent_state: CounterSigningAgentState,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CounterSigningAgentState {
    chain_top: HeaderHash,
    header_seq: u32,
    preflight_signature: Signature,
}

/// A vector of agents to countersign a shared entry.
/// The vector must be sorted to generate a CounterSigningTag.
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct CounterSigningAgentStates(Vec<CounterSigningAgentState>);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HeaderBase {
    Create(CreateBase),
    Update(UpdateBase),
    // @todo - These headers don't have entries so there's nowhere obvious to put the CounterSigningSessionData.
    // Delete(DeleteBase),
    // DeleteLink(DeleteLinkBase),
    // CreateLink(CreateLinkBase),
}

pub struct CreateLinkBase {
    base_address: EntryHash,
    target_address: EntryHash,
    zome_id: ZomeId,
    tag: LinkTag,
}

pub struct DeleteLinkBase {
    base_address: EntryHash,
    link_add_address: HeaderHash,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateBase {
    entry_type: EntryType,
    entry_hash: EntryHash,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateBase {
    original_header_address: HeaderHash,
    original_entry_address: EntryHash,
    entry_type: EntryType,
    entry_hash: EntryHash,
}

pub struct DeleteBase {
    deletes_address: HeaderHash,
    deletes_entry_address: EntryHash,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterSigningSessionData {
    preflight_request: PreflightRequest,
    responses: Vec<CounterSigningAgentState>,
}

impl From<CounterSigningSessionData> for Vec<Header> {
    fn from(session_data: CounterSigningSessionData) -> Self {
        for ((agent, _), agent_state) in session_data.preflight_request.signing_agents.iter().zip(session_data.responses.iter()) {
            match
        }
    }
}