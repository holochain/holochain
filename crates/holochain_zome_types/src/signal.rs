//! App-defined signals

use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

/// A signal emitted by an app via `emit_signal`
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[repr(transparent)]
#[serde(transparent)]
pub struct AppSignal(crate::ExternIO);

impl AppSignal {
    /// Constructor
    pub fn new(extern_io: crate::ExternIO) -> Self {
        Self(extern_io)
    }
    /// Access the inner type
    pub fn into_inner<O, E>(self) -> Result<O, SerializedBytesError>
    where
        SerializedBytesError: From<E>,
        O: TryFrom<SerializedBytes, Error=E>,
    {
        Ok(self.0.try_into()?)
    }
}

/// Remote signal many agents without waiting for responses.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct RemoteSignal {
    /// Agents to send the signal to.
    pub agents: Vec<AgentPubKey>,
    /// The signal to send.
    pub signal: crate::ExternIO,
}
