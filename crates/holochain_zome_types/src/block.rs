use holo_hash::AgentPubKey;

// Everything required for a coordinator to block some agent on the same DNA.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct BlockAgentInput {
    target: AgentPubKey,
    // Reason is literally whatever you want it to be.
    // But unblock must be an exact match.
    #[serde(with = "serde_bytes")]
    reason: Vec<u8>,
}

// Everything required for a coordinator to unblock some agent on the same DNA.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct UnblockAgentInput {
    target: AgentPubKey,
    // Must be exact match for the block or nothing will happen.
    #[serde(with = "serde_bytes")]
    reason: Vec<u8>,
}