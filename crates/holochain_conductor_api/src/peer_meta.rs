use holochain_types::prelude::Timestamp;
use serde::{Deserialize, Serialize};

/// Peer meta info as stored in the peer meta store for a given
/// (peer_url, meta_key) pair
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerMetaInfo {
    pub meta_value: serde_json::Value,
    pub expires_at: Option<Timestamp>,
}
