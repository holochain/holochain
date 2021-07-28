//! Countersigned entries involve preflights between many agents to build a session that is part of the entry.

use crate::prelude::*;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;

/// The timestamps on headers for a session use this offset relative to the session start time.
/// This makes it easier for agents to accept a preflight request with headers that are after their current chain top, after network latency.
pub const SESSION_HEADER_TIME_OFFSET_MILLIS: i64 = 1000;

/// Errors related to the secure primitive macro.
#[derive(Debug, thiserror::Error)]
pub enum CounterSigningError {
    /// Agent index is out of bounds for the signing session.
    #[error("Agent index is out of bounds for the signing session.")]
    AgentIndexOutOfBounds,
}

/// Every countersigning session must complete a full set of headers between the start and end times to be valid.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CounterSigningSessionTimes {
    start: Timestamp,
    end: Timestamp,
}

impl CounterSigningSessionTimes {
    /// Start time accessor.
    pub fn start(&self) -> &Timestamp {
        &self.start
    }

    /// Mutable start time accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn start_mut(&mut self) -> &mut Timestamp {
        &mut self.start
    }

    /// End time accessor.
    pub fn end(&self) -> &Timestamp {
        &self.end
    }

    /// Mutable end time accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn end_mut(&mut self) -> &mut Timestamp {
        &mut self.end
    }
}

/// Every preflight request can have optional arbitrary bytes that can be agreed to.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct PreflightBytes(#[serde(with = "serde_bytes")] Vec<u8>);

/// Agents can have a role specific to each countersigning session.
/// The role is app defined and opaque to the subconscious.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Role(u8);

impl Role {
    /// Constructor.
    pub fn new(role: u8) -> Self {
        Self(role)
    }
}

/// Alias for a list of agents and their roles.
pub type CounterSigningAgents = Vec<(AgentPubKey, Vec<Role>)>;

/// The same PreflightRequest is sent to every agent.
/// Each agent signs this data as part of their PreflightResponse.
/// Every preflight must be identical and signed by every agent for a session to be valid.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
    pub fn signing_agents(&self) -> &CounterSigningAgents {
        &self.signing_agents
    }

    /// Mutable signing agents accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn signing_agents_mut(&mut self) -> &mut CounterSigningAgents {
        &mut self.signing_agents
    }

    /// Enzyme index accessor.
    pub fn enzyme_index(&self) -> &Option<u8> {
        &self.enzyme_index
    }

    /// Mutable enzyme index accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn enzyme_index_mut(&mut self) -> &mut Option<u8> {
        &mut self.enzyme_index
    }

    /// Session times accessor.
    pub fn session_times(&self) -> &CounterSigningSessionTimes {
        &self.session_times
    }

    /// Mutable session times accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn session_times_mut(&mut self) -> &mut CounterSigningSessionTimes {
        &mut self.session_times
    }

    /// Header base accessor.
    pub fn header_base(&self) -> &HeaderBase {
        &self.header_base
    }

    /// Mutable header base accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn header_base_mut(&mut self) -> &mut HeaderBase {
        &mut self.header_base
    }

    /// Preflight bytes accessor.
    pub fn preflight_bytes(&self) -> &PreflightBytes {
        &self.preflight_bytes
    }

    /// Mutable preflight bytes accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn preflight_bytes_mut(&mut self) -> &mut PreflightBytes {
        &mut self.preflight_bytes
    }
}

/// Every agent must send back a preflight response.
/// All the preflight response data is signed by each agent and included in the session data.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct PreflightResponse {
    /// The request this is a response to.
    request: PreflightRequest,
    /// The agent must provide their current chain state, state their position in the preflight and sign everything.
    agent_state: CounterSigningAgentState,
    signature: Signature,
}

impl PreflightResponse {
    /// Constructor.
    pub fn new(
        request: PreflightRequest,
        agent_state: CounterSigningAgentState,
        signature: Signature,
    ) -> Self {
        Self {
            request,
            agent_state,
            signature,
        }
    }

    /// Consistent serialization for the preflight response so it can be signed and the signatures verified.
    pub fn encode_for_signature(&self) -> Result<Vec<u8>, SerializedBytesError> {
        holochain_serialized_bytes::encode(&(&self.request, &self.agent_state))
    }
    /// Request accessor.
    pub fn request(&self) -> &PreflightRequest {
        &self.request
    }

    /// Mutable request accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn request_mut(&mut self) -> &mut PreflightRequest {
        &mut self.request
    }

    /// Agent state accessor.
    pub fn agent_state(&self) -> &CounterSigningAgentState {
        &self.agent_state
    }

    /// Mutable agent state accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn agent_state_mut(&mut self) -> &mut CounterSigningAgentState {
        &mut self.agent_state
    }

    /// Signature accessor.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Mutable signature accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn signature_mut(&mut self) -> &mut Signature {
        &mut self.signature
    }
}

/// Every countersigning agent must sign against their chain state.
/// The chain must be frozen until each agent decides to sign or exit the session.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
    pub fn agent_index(&self) -> &u8 {
        &self.agent_index
    }

    /// Mutable agent index accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn agent_index_mut(&mut self) -> &mut u8 {
        &mut self.agent_index
    }

    /// Chain top accessor.
    pub fn chain_top(&self) -> &HeaderHash {
        &self.chain_top
    }

    /// Mutable chain top accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn chain_top_mut(&mut self) -> &mut HeaderHash {
        &mut self.chain_top
    }

    /// Header seq accessor.
    pub fn header_seq(&self) -> &u32 {
        &self.header_seq
    }

    /// Mutable header seq accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn header_seq_mut(&mut self) -> &mut u32 {
        &mut self.header_seq
    }
}

/// Enum to mirror Header for all the shared data required to build session headers.
/// Does NOT hold any agent specific information.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CreateBase {
    entry_type: EntryType,
    entry_hash: EntryHash,
}

/// Base data for Update headers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct UpdateBase {
    original_header_address: HeaderHash,
    original_entry_address: EntryHash,
    entry_type: EntryType,
    entry_hash: EntryHash,
}

/// All the data required for a countersigning session.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CounterSigningSessionData {
    preflight_request: PreflightRequest,
    responses: Vec<CounterSigningAgentState>,
}

impl Create {
    /// Build an unsigned Create header from session data, shared create data and an agent's state.
    fn from_countersigning_data(
        session_data: &CounterSigningSessionData,
        create_base: &CreateBase,
        agent_state: &CounterSigningAgentState,
    ) -> Result<Self, CounterSigningError> {
        Ok(Create {
            author: session_data
                .preflight_request
                .signing_agents
                .get(agent_state.agent_index as usize)
                .ok_or(CounterSigningError::AgentIndexOutOfBounds)?
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
        })
    }
}

impl Update {
    /// Build an unsigned Update header from session data, shared update data and an agent's state.
    fn from_countersigning_data(
        session_data: &CounterSigningSessionData,
        update_base: &UpdateBase,
        agent_state: &CounterSigningAgentState,
    ) -> Result<Self, CounterSigningError> {
        Ok(Update {
            author: session_data
                .preflight_request
                .signing_agents
                .get(agent_state.agent_index as usize)
                .ok_or(CounterSigningError::AgentIndexOutOfBounds)?
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
        })
    }
}

/// All the data required for a countersigning session.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CounterSigningSessionData {
    preflight_request: PreflightRequest,
    responses: Vec<(CounterSigningAgentState, Signature)>,
}

impl CounterSigningSessionData {
    /// Attempt to map countersigning session data to a set of headers.
    /// A given countersigning session always maps to the same ordered set of headers or an error.
    /// Note the headers are not signed as the intent is to build headers for other agents without their private keys.
    pub fn build_header_set(&self) -> Result<Vec<Header>, CounterSigningError> {
        let mut headers = vec![];
        for (agent_state, _response_signature) in self.responses.iter() {
            match self.preflight_request.header_base {
                HeaderBase::Create(ref create_base) => {
                    headers.push(Header::Create(Create::from_countersigning_data(
                        self,
                        create_base,
                        agent_state,
                    )?));
                }
                HeaderBase::Update(ref update_base) => {
                    headers.push(Header::Update(Update::from_countersigning_data(
                        self,
                        update_base,
                        agent_state,
                    )?));
                }
            }
        }
        Ok(headers)
    }

    /// Accessor to the preflight request.
    pub fn preflight_request(&self) -> &PreflightRequest {
        &self.preflight_request
    }

    /// Mutable preflight_request accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn preflight_request_mut(&mut self) -> &mut PreflightRequest {
        &mut self.preflight_request
    }

    /// Accessor to responses.
    pub fn responses(&self) -> &Vec<(CounterSigningAgentState, Signature)> {
        &self.responses
    }

    /// Mutable responses accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn responses_mut(&mut self) -> &mut Vec<(CounterSigningAgentState, Signature)> {
        &mut self.responses
    }
}
