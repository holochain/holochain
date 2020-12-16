//! App-defined signals

use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

/// A signal emitted by an app via `emit_signal`
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
#[repr(transparent)]
#[serde(transparent)]
pub struct AppSignal(SerializedBytes);

impl AppSignal {
    /// Constructor
    pub fn new(sb: SerializedBytes) -> Self {
        Self(sb)
    }
}

/// Remote signal many agents without waiting for responses.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct RemoteSignal {
    /// Agents to send the signal to.
    pub agents: Vec<AgentPubKey>,
    /// The signal to send.
    pub signal: SerializedBytes,
}
