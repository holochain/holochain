//! stub module for dht ops

use holochain_serialized_bytes::prelude::*;

/// Stub / placeholder DhtOp enum
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub enum DhtOp {
    /// stub / placeholder variant for working with DhtOps
    Stub,
}
