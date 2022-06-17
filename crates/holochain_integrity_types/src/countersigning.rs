//! Countersigned entries involve preflights between many agents to build a session that is part of the entry.

use std::iter::FromIterator;
use std::time::Duration;

use crate::prelude::*;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holochain_serialized_bytes::SerializedBytesError;

/// The timestamps on actions for a session use this offset relative to the session start time.
/// This makes it easier for agents to accept a preflight request with actions that are after their current chain top, after network latency.
pub const SESSION_ACTION_TIME_OFFSET: Duration = Duration::from_millis(1000);

/// Maximum time in the future the session start can be in the opinion of the participating agent.
/// As the action will be `SESSION_ACTION_TIME_OFFSET` after the session start we include that here.
pub const SESSION_TIME_FUTURE_MAX: Duration =
    Duration::from_millis(5000 + SESSION_ACTION_TIME_OFFSET.as_millis() as u64);

/// Need at least two to countersign.
pub const MIN_COUNTERSIGNING_AGENTS: usize = 2;
/// 8 seems like a reasonable limit of agents to countersign.
pub const MAX_COUNTERSIGNING_AGENTS: usize = 8;

pub use error::CounterSigningError;
mod error;

/// Every countersigning session must complete a full set of actions between the start and end times to be valid.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CounterSigningSessionTimes {
    start: Timestamp,
    end: Timestamp,
}

impl CounterSigningSessionTimes {
    /// Fallible constructor.
    pub fn try_new(start: Timestamp, end: Timestamp) -> Result<Self, CounterSigningError> {
        let session_times = Self { start, end };
        session_times.check_integrity()?;
        Ok(session_times)
    }

    /// Verify the difference between the end and start time is larger than the session action time offset.
    pub fn check_integrity(&self) -> Result<(), CounterSigningError> {
        let times_are_valid = &Timestamp::from_micros(0) < self.start()
            && self.start()
                <= &(self.end() - SESSION_ACTION_TIME_OFFSET).map_err(|_| {
                    CounterSigningError::CounterSigningSessionTimes((*self).clone())
                })?;
        if times_are_valid {
            Ok(())
        } else {
            Err(CounterSigningError::CounterSigningSessionTimes(
                (*self).clone(),
            ))
        }
    }

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
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct PreflightBytes(#[serde(with = "serde_bytes")] pub Vec<u8>);

/// Agents can have a role specific to each countersigning session.
/// The role is app defined and opaque to the subconscious.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Role(pub u8);

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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct PreflightRequest {
    /// The hash of the app entry, as if it were not countersigned.
    /// The final entry hash will include the countersigning session.
    app_entry_hash: EntryHash,
    /// The agents that are participating in this countersignature session.
    signing_agents: CounterSigningAgents,
    /// The agent that must receive and include all other actions in their own action.
    /// @todo implement enzymes
    enzyme_index: Option<u8>,
    /// The session times.
    /// Session actions must all have the same timestamp, which is the session offset.
    session_times: CounterSigningSessionTimes,
    /// The action information that is shared by all agents.
    /// Contents depend on the action type, create, update, etc.
    action_base: ActionBase,
    /// The preflight bytes for session.
    preflight_bytes: PreflightBytes,
}

impl PreflightRequest {
    /// Fallible constructor.
    pub fn try_new(
        app_entry_hash: EntryHash,
        signing_agents: CounterSigningAgents,
        enzyme_index: Option<u8>,
        session_times: CounterSigningSessionTimes,
        action_base: ActionBase,
        preflight_bytes: PreflightBytes,
    ) -> Result<Self, CounterSigningError> {
        let preflight_request = Self {
            app_entry_hash,
            signing_agents,
            enzyme_index,
            session_times,
            action_base,
            preflight_bytes,
        };
        preflight_request.check_integrity()?;
        Ok(preflight_request)
    }
    /// Combined integrity checks.
    pub fn check_integrity(&self) -> Result<(), CounterSigningError> {
        self.check_enzyme_index()?;
        self.session_times().check_integrity()?;
        self.check_agents()?;
        Ok(())
    }

    /// Verify there are no duplicate agents to sign.
    pub fn check_agents_dupes(&self) -> Result<(), CounterSigningError> {
        let v: Vec<AgentPubKey> = self
            .signing_agents()
            .iter()
            .map(|(agent, _roles)| agent.clone())
            .collect();
        if std::collections::HashSet::<AgentPubKey>::from_iter(v.clone()).len()
            == self.signing_agents().len()
        {
            Ok(())
        } else {
            Err(CounterSigningError::AgentsDupes(v))
        }
    }

    /// Verify the number of signing agents is within the correct range.
    pub fn check_agents_len(&self) -> Result<(), CounterSigningError> {
        if MIN_COUNTERSIGNING_AGENTS <= self.signing_agents().len()
            && self.signing_agents().len() <= MAX_COUNTERSIGNING_AGENTS
        {
            Ok(())
        } else {
            Err(CounterSigningError::AgentsLength(
                self.signing_agents().len(),
            ))
        }
    }

    /// Verify the preflight request agents.
    pub fn check_agents(&self) -> Result<(), CounterSigningError> {
        self.check_agents_dupes()?;
        self.check_agents_len()?;
        Ok(())
    }

    /// Verify the enzyme index is in bounds of the signing agent if set.
    pub fn check_enzyme_index(&self) -> Result<(), CounterSigningError> {
        match self.enzyme_index() {
            Some(index) => {
                if (*index as usize) < self.signing_agents().len() {
                    Ok(())
                } else {
                    Err(CounterSigningError::EnzymeIndex(
                        self.signing_agents().len(),
                        *index as usize,
                    ))
                }
            }
            None => Ok(()),
        }
    }

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

    /// Action base accessor.
    pub fn action_base(&self) -> &ActionBase {
        &self.action_base
    }

    /// Mutable action base accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn action_base_mut(&mut self) -> &mut ActionBase {
        &mut self.action_base
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct PreflightResponse {
    /// The request this is a response to.
    request: PreflightRequest,
    /// The agent must provide their current chain state, state their position in the preflight and sign everything.
    agent_state: CounterSigningAgentState,
    signature: Signature,
}

impl PreflightResponse {
    /// Fallible constructor.
    pub fn try_new(
        request: PreflightRequest,
        agent_state: CounterSigningAgentState,
        signature: Signature,
    ) -> Result<Self, CounterSigningError> {
        let preflight_response = Self {
            request,
            agent_state,
            signature,
        };
        preflight_response.check_integrity()?;
        Ok(preflight_response)
    }

    /// Combined preflight response validation call.
    pub fn check_integrity(&self) -> Result<(), CounterSigningError> {
        self.request().check_integrity()
    }

    /// Serialization for signing of the signable field data only.
    pub fn encode_fields_for_signature(
        request: &PreflightRequest,
        agent_state: &CounterSigningAgentState,
    ) -> Result<Vec<u8>, SerializedBytesError> {
        holochain_serialized_bytes::encode(&(request, agent_state))
    }

    /// Consistent serialization for the preflight response so it can be signed and the signatures verified.
    pub fn encode_for_signature(&self) -> Result<Vec<u8>, SerializedBytesError> {
        Self::encode_fields_for_signature(&self.request, &self.agent_state)
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

/// A preflight request can be accepted, or invalid, or valid but the local agent cannot accept it.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum PreflightRequestAcceptance {
    /// Preflight request accepted.
    Accepted(PreflightResponse),
    /// The preflight request start time is too far in the future for the agent.
    UnacceptableFutureStart,
    /// The preflight request does not include the agent.
    UnacceptableAgentNotFound,
    /// The preflight request is invalid as it failed some integrity check.
    Invalid(String),
}

/// Every countersigning agent must sign against their chain state.
/// The chain must be frozen until each agent decides to sign or exit the session.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CounterSigningAgentState {
    /// The index of the agent in the preflight request agent vector.
    agent_index: u8,
    /// The current (frozen) top of the agent's local chain.
    chain_top: ActionHash,
    /// The action sequence of the agent's chain top.
    action_seq: u32,
}

impl CounterSigningAgentState {
    /// Constructor.
    pub fn new(agent_index: u8, chain_top: ActionHash, action_seq: u32) -> Self {
        Self {
            agent_index,
            chain_top,
            action_seq,
        }
    }

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
    pub fn chain_top(&self) -> &ActionHash {
        &self.chain_top
    }

    /// Mutable chain top accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn chain_top_mut(&mut self) -> &mut ActionHash {
        &mut self.chain_top
    }

    /// Action seq accessor.
    pub fn action_seq(&self) -> &u32 {
        &self.action_seq
    }

    /// Mutable action seq accessor for testing.
    #[cfg(feature = "test_utils")]
    pub fn action_seq_mut(&mut self) -> &mut u32 {
        &mut self.action_seq
    }
}

/// Enum to mirror Action for all the shared data required to build session actions.
/// Does NOT hold any agent specific information.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum ActionBase {
    /// Mirrors Action::Create.
    Create(CreateBase),
    /// Mirrors Action::Update.
    Update(UpdateBase),
    // @todo - These actions don't have entries so there's nowhere obvious to put the CounterSigningSessionData.
    // Delete(DeleteBase),
    // DeleteLink(DeleteLinkBase),
    // CreateLink(CreateLinkBase),
}

/// Base data for Create actions.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CreateBase {
    entry_type: EntryType,
}

impl CreateBase {
    /// Constructor.
    pub fn new(entry_type: EntryType) -> Self {
        Self { entry_type }
    }
}

/// Base data for Update actions.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct UpdateBase {
    original_action_address: ActionHash,
    original_entry_address: EntryHash,
    entry_type: EntryType,
}

impl Action {
    /// Construct an action from the ActionBase and associated session data.
    pub fn from_countersigning_data(
        entry_hash: EntryHash,
        session_data: &CounterSigningSessionData,
        author: AgentPubKey,
    ) -> Result<Self, CounterSigningError> {
        let agent_state = session_data.agent_state_for_agent(&author)?;
        Ok(match session_data.preflight_request().action_base() {
            ActionBase::Create(create_base) => Action::Create(Create {
                author,
                timestamp: session_data.to_timestamp(),
                action_seq: agent_state.action_seq + 1,
                prev_action: agent_state.chain_top.clone(),
                entry_type: create_base.entry_type.clone(),
                entry_hash,
            }),
            ActionBase::Update(update_base) => Action::Update(Update {
                author,
                timestamp: session_data.to_timestamp(),
                action_seq: agent_state.action_seq + 1,
                prev_action: agent_state.chain_top.clone(),
                original_action_address: update_base.original_action_address.clone(),
                original_entry_address: update_base.original_entry_address.clone(),
                entry_type: update_base.entry_type.clone(),
                entry_hash,
            }),
        })
    }
}

/// All the data required for a countersigning session.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CounterSigningSessionData {
    preflight_request: PreflightRequest,
    responses: Vec<(CounterSigningAgentState, Signature)>,
}

impl CounterSigningSessionData {
    /// Attempt to build session data from a vector of responses.
    pub fn try_from_responses(
        responses: Vec<PreflightResponse>,
    ) -> Result<Self, CounterSigningError> {
        let preflight_response = responses
            .get(0)
            .ok_or(CounterSigningError::MissingResponse)?
            .to_owned();
        let responses: Vec<(CounterSigningAgentState, Signature)> = responses
            .into_iter()
            .map(|response| (response.agent_state.clone(), response.signature))
            .collect();
        Ok(Self {
            preflight_request: preflight_response.request,
            responses,
        })
    }

    /// Get the agent state for a specific agent.
    pub fn agent_state_for_agent(
        &self,
        agent: &AgentPubKey,
    ) -> Result<&CounterSigningAgentState, CounterSigningError> {
        match self
            .preflight_request
            .signing_agents()
            .iter()
            .position(|(pubkey, _)| pubkey == agent)
        {
            Some(agent_index) => match self.responses.get(agent_index as usize) {
                Some((agent_state, _)) => Ok(agent_state),
                None => Err(CounterSigningError::AgentIndexOutOfBounds),
            },
            None => Err(CounterSigningError::AgentIndexOutOfBounds),
        }
    }

    /// Attempt to map countersigning session data to a set of actions.
    /// A given countersigning session always maps to the same ordered set of actions or an error.
    /// Note the actions are not signed as the intent is to build actions for other agents without their private keys.
    pub fn build_action_set(
        &self,
        entry_hash: EntryHash,
    ) -> Result<Vec<Action>, CounterSigningError> {
        let mut actions = vec![];
        for (agent, _role) in self.preflight_request.signing_agents().iter() {
            actions.push(Action::from_countersigning_data(
                entry_hash.clone(),
                self,
                agent.clone(),
            )?);
        }
        Ok(actions)
    }

    /// Fallible constructor.
    pub fn try_new(
        preflight_request: PreflightRequest,
        responses: Vec<(CounterSigningAgentState, Signature)>,
    ) -> Result<Self, CounterSigningError> {
        let session_data = Self {
            preflight_request,
            responses,
        };
        session_data.check_integrity()?;
        Ok(session_data)
    }

    /// Combines all integrity checks.
    pub fn check_integrity(&self) -> Result<(), CounterSigningError> {
        self.check_responses_indexes()
    }

    /// Check that the countersigning session data responses all have the
    /// correct indexes.
    pub fn check_responses_indexes(&self) -> Result<(), CounterSigningError> {
        if self.preflight_request().signing_agents().len() != self.responses().len() {
            Err(CounterSigningError::CounterSigningSessionResponsesLength(
                self.responses().len(),
                self.preflight_request().signing_agents().len(),
            ))
        } else {
            for (i, (response, _response_signature)) in self.responses().iter().enumerate() {
                if *response.agent_index() as usize != i {
                    return Err(CounterSigningError::CounterSigningSessionResponsesOrder(
                        *response.agent_index(),
                        i,
                    ));
                }
            }
            Ok(())
        }
    }

    /// Construct a Timestamp from countersigning session data.
    /// Ostensibly used for the Action because the session itself covers a time range.
    pub fn to_timestamp(&self) -> Timestamp {
        (self.preflight_request().session_times().start() + SESSION_ACTION_TIME_OFFSET)
            .unwrap_or(Timestamp::MAX)
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

    /// Get all the agents signing for this session.
    pub fn signing_agents(&self) -> impl Iterator<Item = &AgentPubKey> {
        self.preflight_request.signing_agents.iter().map(|(a, _)| a)
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

#[cfg(test)]
pub mod test {
    use crate::CounterSigningAgentState;
    use crate::CounterSigningSessionData;
    use crate::Signature;
    use holo_hash::AgentPubKey;

    use super::CounterSigningError;
    use super::CounterSigningSessionTimes;
    use super::PreflightRequest;
    use super::SESSION_ACTION_TIME_OFFSET;
    use crate::Role;
    use arbitrary::Arbitrary;

    #[test]
    pub fn test_check_countersigning_session_times() {
        let mut u = arbitrary::Unstructured::new(&[0; 1000]);
        let mut session_times = CounterSigningSessionTimes::arbitrary(&mut u).unwrap();

        // Zero start and end won't pass.
        assert!(matches!(
            session_times.check_integrity(),
            Err(CounterSigningError::CounterSigningSessionTimes(_))
        ));

        // Shifting the end forward 1 milli won't help.
        *session_times.end_mut() =
            (session_times.end() + core::time::Duration::from_millis(1)).unwrap();
        assert!(matches!(
            session_times.check_integrity(),
            Err(CounterSigningError::CounterSigningSessionTimes(_))
        ));

        // Shifting the end forward by the session offset will _almost_ fix it.
        *session_times.end_mut() = (session_times.end() + SESSION_ACTION_TIME_OFFSET).unwrap();
        assert!(matches!(
            session_times.check_integrity(),
            Err(CounterSigningError::CounterSigningSessionTimes(_))
        ));

        // making the the start non-zero should fix it.
        *session_times.start_mut() =
            (session_times.start() + core::time::Duration::from_millis(1)).unwrap();
        assert_eq!(session_times.check_integrity().unwrap(), (),);

        // making the diff between start and end less than the action offset will break it again.
        *session_times.start_mut() =
            (session_times.start() + core::time::Duration::from_millis(1)).unwrap();
        assert!(matches!(
            session_times.check_integrity(),
            Err(CounterSigningError::CounterSigningSessionTimes(_))
        ));
    }

    #[test]
    pub fn test_check_countersigning_preflight_request_enzyme_index() {
        let mut u = arbitrary::Unstructured::new(&[0; 1000]);
        let mut preflight_request = PreflightRequest::arbitrary(&mut u).unwrap();

        // None is always a pass.
        assert_eq!(preflight_request.check_enzyme_index().unwrap(), ());

        let alice = AgentPubKey::arbitrary(&mut u).unwrap();
        (*preflight_request.signing_agents_mut()).push((alice.clone(), vec![]));

        // 0 is the first signing agent so is a valid enzyme.
        *preflight_request.enzyme_index_mut() = Some(0);

        assert_eq!(preflight_request.check_enzyme_index().unwrap(), (),);

        // 1 is out of bounds for zero signing agents.
        *preflight_request.enzyme_index_mut() = Some(1);

        assert!(matches!(
            preflight_request.check_enzyme_index(),
            Err(CounterSigningError::EnzymeIndex(_, _))
        ));
    }

    #[test]
    pub fn test_check_countersigning_preflight_request_agents_len() {
        let mut u = arbitrary::Unstructured::new(&[0; 1000]);
        let mut preflight_request = PreflightRequest::arbitrary(&mut u).unwrap();

        // Empty is a fail.
        assert!(matches!(
            preflight_request.check_agents_len(),
            Err(CounterSigningError::AgentsLength(_))
        ));

        // One signer is a fail.
        let alice = AgentPubKey::arbitrary(&mut u).unwrap();
        (*preflight_request.signing_agents_mut()).push((alice.clone(), vec![]));

        assert!(matches!(
            preflight_request.check_agents_len(),
            Err(CounterSigningError::AgentsLength(_))
        ));

        // Two signers is a pass.
        let bob = AgentPubKey::arbitrary(&mut u).unwrap();
        (*preflight_request.signing_agents_mut()).push((bob.clone(), vec![]));

        assert_eq!(preflight_request.check_agents_len().unwrap(), (),);
    }

    #[test]
    pub fn test_check_countersigning_preflight_request_agents_dupes() {
        let data: Vec<_> = (0u8..255).cycle().take(1000).collect();
        let mut u = arbitrary::Unstructured::new(&data);
        let mut preflight_request = PreflightRequest::arbitrary(&mut u).unwrap();

        let alice = AgentPubKey::arbitrary(&mut u).unwrap();
        let bob = AgentPubKey::arbitrary(&mut u).unwrap();

        assert_eq!(preflight_request.check_agents_dupes().unwrap(), (),);

        (*preflight_request.signing_agents_mut()).push((alice.clone(), vec![]));
        assert_eq!(preflight_request.check_agents_dupes().unwrap(), (),);

        (*preflight_request.signing_agents_mut()).push((bob.clone(), vec![]));
        assert_eq!(preflight_request.check_agents_dupes().unwrap(), (),);

        // Another alice is a dupe, even if roles are different.
        (*preflight_request.signing_agents_mut()).push((alice.clone(), vec![Role::new(0_u8)]));
        assert!(matches!(
            preflight_request.check_agents_dupes(),
            Err(CounterSigningError::AgentsDupes(_))
        ));
    }

    #[test]
    pub fn test_check_countersigning_session_data_responses_indexes() {
        let mut u = arbitrary::Unstructured::new(&[0; 1000]);
        let mut session_data = CounterSigningSessionData::arbitrary(&mut u).unwrap();

        let alice = AgentPubKey::arbitrary(&mut u).unwrap();
        let bob = AgentPubKey::arbitrary(&mut u).unwrap();

        // When everything is empty the indexes line up by default.
        assert_eq!(session_data.check_responses_indexes().unwrap(), ());

        // When the signing agents and responses are out of sync it must error.
        (*session_data.preflight_request_mut().signing_agents_mut()).push((alice.clone(), vec![]));
        assert!(matches!(
            session_data.check_responses_indexes(),
            Err(CounterSigningError::CounterSigningSessionResponsesLength(
                _,
                _
            ))
        ));

        // When signing agents indexes are not in the correct order it must error.
        (*session_data.preflight_request_mut().signing_agents_mut()).push((bob.clone(), vec![]));

        let alice_state = CounterSigningAgentState::arbitrary(&mut u).unwrap();
        let alice_signature = Signature::arbitrary(&mut u).unwrap();
        let mut bob_state = CounterSigningAgentState::arbitrary(&mut u).unwrap();
        let bob_signature = Signature::arbitrary(&mut u).unwrap();

        (*session_data.responses_mut()).push((alice_state, alice_signature));
        (*session_data.responses_mut()).push((bob_state.clone(), bob_signature.clone()));

        assert!(matches!(
            session_data.check_responses_indexes(),
            Err(CounterSigningError::CounterSigningSessionResponsesOrder(
                _,
                _
            ))
        ));

        *bob_state.agent_index_mut() = 1;
        (*session_data.responses_mut()).pop();
        (*session_data.responses_mut()).push((bob_state, bob_signature));
        assert_eq!(session_data.check_responses_indexes().unwrap(), (),);
    }
}
