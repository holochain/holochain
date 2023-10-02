use crate::CapGrant;
use crate::FunctionName;
use crate::Timestamp;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

pub use holochain_integrity_types::info::*;

/// The struct containing all information about the executing agent's identity.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct AgentInfo {
    /// The current agent's pubkey at genesis.
    /// Always found at index 2 in the source chain.
    pub agent_initial_pubkey: AgentPubKey,
    /// The current agent's current pubkey.
    /// Same as the initial pubkey if it has never been changed.
    /// The agent can revoke an old key and replace it with a new one, the latest appears here.
    pub agent_latest_pubkey: AgentPubKey,
    pub chain_head: (ActionHash, u32, Timestamp),
}

impl AgentInfo {
    pub fn new(
        agent_initial_pubkey: AgentPubKey,
        agent_latest_pubkey: AgentPubKey,
        chain_head: (ActionHash, u32, Timestamp),
    ) -> Self {
        Self {
            agent_initial_pubkey,
            agent_latest_pubkey,
            chain_head,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallInfo {
    pub provenance: AgentPubKey,
    pub function_name: FunctionName,
    /// Chain head as at the call start.
    /// This will not change within a call even if the chain is written to.
    pub as_at: (ActionHash, u32, Timestamp),
    pub cap_grant: CapGrant,
}
