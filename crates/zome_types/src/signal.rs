//! App-defined signals

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
