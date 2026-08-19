use holochain_types::prelude::Timestamp;
use serde::{Deserialize, Serialize};

/// Peer meta info as stored in the peer meta store for a given
/// (peer_url, meta_key) pair
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "api/admin/types.ts"))]
pub struct PeerMetaInfo {
    /// The value stored for this meta key, an arbitrary JSON value.
    #[cfg_attr(feature = "ts_rs", ts(type = "unknown"))]
    pub meta_value: serde_json::Value,
    /// When this meta entry expires, if it has an expiry.
    pub expires_at: Option<Timestamp>,
}
