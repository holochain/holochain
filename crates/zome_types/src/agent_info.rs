use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

/// The struct containing all global agent values accessible to a zome
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct AgentInfo {
    pub agent_initial_pubkey: AgentPubKey,
    pub agent_latest_pubkey: AgentPubKey,
}
