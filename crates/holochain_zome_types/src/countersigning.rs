//! Countersigned entries involve preflights between many agents to build a session that is part of the entry.

use crate::prelude::*;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;

/// The timestamps on headers for a session use this offset relative to the session start time.
/// This makes it easier for agents to accept a preflight request with headers that are after their current chain top, after network latency.
pub const SESSION_HEADER_TIME_OFFSET_MILLIS: i64 = 1000;

/// Every countersigning session must complete a full set of headers between the start and end times to be valid.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterSigningSessionTimes {
    start: Timestamp,
    end: Timestamp,
}

impl CounterSigningSessionTimes {
    /// Start time accessor.
    pub fn as_start_ref(&self) -> &Timestamp {
        &self.start
    }

    /// End time accessor.
    pub fn as_end_ref(&self) -> &Timestamp {
        &self.end
    }
}

/// Every preflight request can have optional arbitrary bytes that can be agreed to.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct PreflightBytes(#[serde(with = "serde_bytes")] Vec<u8>);

/// Agents can have a role specific to each countersigning session.
/// The role is app defined and opaque to the subconscious.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Role(u8);

/// Alias for a list of agents and their roles.
pub type CounterSigningAgents = Vec<(AgentPubKey, Vec<Role>)>;

/// The same PreflightRequest is sent to every agent.
/// Each agent signs this data as part of their PreflightResponse.
/// Every preflight must be identical and signed by every agent for a session to be valid.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreflightRequest {
    /// The agents that are participating in this countersignature session.
    signing_agents: CounterSigningAgents,
    /// The agent that must receive and include all other headers in their own header.
    /// @todo implement enzymes
    enzyme_index: Option<u8>,
    /// The session times.
    /// Session headers must all have the same timestamp, which is the session offset.
    session_times: CounterSigningSessionTimes,
    /// The header information that is shared by all agents.
    /// Contents depend on the header type, create, update, etc.
    header_base: HeaderBase,
    /// The preflight bytes for session.
    preflight_bytes: PreflightBytes,
}

impl PreflightRequest {
    /// Signing agents accessor.
    pub fn signing_agents_ref(&self) -> &CounterSigningAgents {
        &self.signing_agents
    }

    /// Enzyme index accessor.
    pub fn enzyme_index_ref(&self) -> &Option<u8> {
        &self.enzyme_index
    }

    /// Session times accessor.
    pub fn session_times_ref(&self) -> &CounterSigningSessionTimes {
        &self.session_times
    }

    /// Header base accessor.
    pub fn header_base_ref(&self) -> &HeaderBase {
        &self.header_base
    }

    /// Preflight bytes accessor.
    pub fn preflight_bytes_ref(&self) -> &PreflightBytes {
        &self.preflight_bytes
    }
}

/// Every agent must send back a preflight response.
/// All the preflight response data is signed by each agent and included in the session data.
pub struct PreflightResponse {
    /// The request this is a response to.
    request: PreflightRequest,
    /// The agent must provide their current chain state, state their position in the preflight and sign everything.
    agent_state: CounterSigningAgentState,
    signature: Signature,
}

impl PreflightResponse {
    pub fn verify_signature(&self) -> Result<bool> {
        self.request.signing_agents[self.agent_state.agent_index as usize]
            .verify_signature(self.signature, (self.request, self.agent_state))
    }
}

impl PreflightResponse {
    /// Request accessor.
    pub fn request_ref(&self) -> &PreflightRequest {
        &self.request
    }

    /// Agent state accessor.
    pub fn agent_state_ref(&self) -> &CounterSigningAgentState {
        &self.agent_state
    }
}

/// Every countersigning agent must sign against their chain state.
/// The chain must be frozen until each agent decides to sign or exit the session.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CounterSigningAgentState {
    /// The index of the agent in the preflight request agent vector.
    agent_index: u8,
    /// The current (frozen) top of the agent's local chain.
    chain_top: HeaderHash,
    /// The header sequence of the agent's chain top.
    header_seq: u32,
}

impl CounterSigningAgentState {
    /// Agent index accessor.
    pub fn agent_index(&self) -> u8 {
        self.agent_index
    }

    /// Chain top accessor.
    pub fn chain_top_ref(&self) -> &HeaderHash {
        &self.chain_top
    }

    /// Header seq accessor.
    pub fn header_seq(&self) -> u32 {
        self.header_seq
    }

    /// Signature accessor.
    pub fn signature_ref(&self) -> &Signature {
        &self.signature
    }
}

/// A vector of agents to countersign a shared entry.
/// The vector must be sorted to generate a CounterSigningTag.
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct CounterSigningAgentStates(Vec<CounterSigningAgentState>);

/// Enum to mirror Header for all the shared data required to build session headers.
/// Does NOT hold any agent specific information.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HeaderBase {
    /// Mirrors Header::Create.
    Create(CreateBase),
    /// Mirrors Header::Update.
    Update(UpdateBase),
    // @todo - These headers don't have entries so there's nowhere obvious to put the CounterSigningSessionData.
    // Delete(DeleteBase),
    // DeleteLink(DeleteLinkBase),
    // CreateLink(CreateLinkBase),
}

/// Base data for Create headers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateBase {
    entry_type: EntryType,
    entry_hash: EntryHash,
}

/// Base data for Update headers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateBase {
    original_header_address: HeaderHash,
    original_entry_address: EntryHash,
    entry_type: EntryType,
    entry_hash: EntryHash,
}

/// All the data required for a countersigning session.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterSigningSessionData {
    preflight_request: PreflightRequest,
    responses: Vec<CounterSigningAgentState>,
    signatures: Vec<Signature>,
}

/// Build an unsigned Create header from session data, shared create data and an agent's state.
impl
    From<(
        &CounterSigningSessionData,
        &CreateBase,
        &CounterSigningAgentState,
    )> for Create
{
    fn from(
        (session_data, create_base, agent_state): (
            &CounterSigningSessionData,
            &CreateBase,
            &CounterSigningAgentState,
        ),
    ) -> Self {
        Create {
            author: session_data.preflight_request.signing_agents[agent_state.agent_index as usize]
                .0
                .clone(),
            timestamp: Timestamp(
                session_data
                    .preflight_request
                    .session_times
                    .start
                    .0
                    .checked_add(SESSION_HEADER_TIME_OFFSET_MILLIS)
                    .unwrap_or(i64::MAX),
                session_data.preflight_request.session_times.start.1,
            ),
            header_seq: agent_state.header_seq,
            prev_header: agent_state.chain_top.clone(),
            entry_type: create_base.entry_type.clone(),
            entry_hash: create_base.entry_hash.clone(),
        }
    }
}

/// Build an unsigned Update header from session data, shared update data and an agent's state.
impl
    From<(
        &CounterSigningSessionData,
        &UpdateBase,
        &CounterSigningAgentState,
    )> for Update
{
    fn from(
        (session_data, update_base, agent_state): (
            &CounterSigningSessionData,
            &UpdateBase,
            &CounterSigningAgentState,
        ),
    ) -> Self {
        Update {
            author: session_data.preflight_request.signing_agents[agent_state.agent_index as usize]
                .0
                .clone(),
            timestamp: Timestamp(
                session_data
                    .preflight_request
                    .session_times
                    .start
                    .0
                    .checked_add(SESSION_HEADER_TIME_OFFSET_MILLIS)
                    .unwrap_or(i64::MAX),
                session_data.preflight_request.session_times.start.1,
            ),
            header_seq: agent_state.header_seq,
            prev_header: agent_state.chain_top.clone(),
            original_header_address: update_base.original_header_address.clone(),
            original_entry_address: update_base.original_entry_address.clone(),
            entry_type: update_base.entry_type.clone(),
            entry_hash: update_base.entry_hash.clone(),
        }
    }
}

impl From<CounterSigningSessionData> for Vec<Header> {
    fn from(session_data: CounterSigningSessionData) -> Self {
        let mut headers = vec![];
        for agent_state in session_data.responses.iter() {
            match session_data.preflight_request.header_base {
                HeaderBase::Create(ref create_base) => {
                    headers.push(Header::Create(Create::from((
                        &session_data,
                        create_base,
                        agent_state,
                    ))));
                }
                HeaderBase::Update(ref update_base) => {
                    headers.push(Header::Update(Update::from((
                        &session_data,
                        update_base,
                        agent_state,
                    ))));
                }
            }
        }
        headers
    }
}
