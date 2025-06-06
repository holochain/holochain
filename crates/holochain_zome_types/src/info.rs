use crate::prelude::*;
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
    pub chain_head: (ActionHash, u32, Timestamp),
}

impl AgentInfo {
    pub fn new(
        agent_initial_pubkey: AgentPubKey,
        chain_head: (ActionHash, u32, Timestamp),
    ) -> Self {
        Self {
            agent_initial_pubkey,
            chain_head,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallInfo {
    /// The provenance identifies the agent who made the call.
    /// This is the author of the chain for local calls, and the assignee of a capability for remote calls.
    pub provenance: AgentPubKey,
    /// The function name that was the entrypoint into the wasm.
    pub function_name: FunctionName,
    /// Chain head as at the call start.
    /// This will not change within a call even if the chain is written to.
    pub as_at: (ActionHash, u32, Timestamp),
    /// The capability grant used to authorize the call.
    pub cap_grant: CapGrant,
}
