use holo_hash::AgentPubKey;
use holochain_integrity_types::Timestamp;

// Everything required for a coordinator to block some agent on the same DNA.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct BlockAgentInput {
    pub target: AgentPubKey,
    // Reason is literally whatever you want it to be.
    // But unblock must be an exact match.
    #[serde(with = "serde_bytes")]
    pub reason: Vec<u8>,
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
}