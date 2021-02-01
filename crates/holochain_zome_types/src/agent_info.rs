use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

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
}
