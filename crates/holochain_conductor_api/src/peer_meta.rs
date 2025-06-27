use holochain_types::prelude::Timestamp;
use serde::{Deserialize, Serialize};

/// Agent meta info as stored in the peer meta store for a given
/// (peer_url, meta_key) pair
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentMetaInfo {
    pub meta_value: serde_json::Value,
    pub expires_at: Timestamp,
}
