use crate::conductor::{api::error::ConductorApiError, error::ConductorError};
use holochain_serialized_bytes::prelude::*;
use holochain_serialized_bytes::SerializedBytesError;

/// Interface Error Type
#[derive(Debug, thiserror::Error)]
pub enum InterfaceError {
    SerializedBytes(#[from] SerializedBytesError),
    JoinError(#[from] tokio::task::JoinError),
    SignalReceive(tokio::sync::broadcast::RecvError),
    RequestHandler(ConductorError),
    UnexpectedMessage(String),
    SendError,
    Other(String),
    Closed,
    // FIXME: update error types in holochain_websocket to use a more specific
    // type than io::Error
    IoTodo(#[from] std::io::Error),
}

impl std::fmt::Display for InterfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<String> for InterfaceError {
    fn from(o: String) -> Self {
        InterfaceError::Other(o)
    }
}

impl From<futures::channel::mpsc::SendError> for InterfaceError {
    fn from(_: futures::channel::mpsc::SendError) -> Self {
        InterfaceError::SendError
    }
}

/// Interface Result Type
pub type InterfaceResult<T> = Result<T, InterfaceError>;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AdminInterfaceErrorKind {
    Serialization,
    Cell,
    Conductor,
    Io,
    RealConductor,
    BadRequest,
    Other,
    Cache,
}

impl From<InterfaceError> for AdminInterfaceErrorKind {
    fn from(error: InterfaceError) -> Self {
        use AdminInterfaceErrorKind::*;
        match error {
            InterfaceError::SerializedBytes(_) => Serialization,
            InterfaceError::JoinError(_) => RealConductor,
            InterfaceError::SignalReceive(_) => RealConductor,
            InterfaceError::RequestHandler(_) => Conductor,
            InterfaceError::UnexpectedMessage(_) => BadRequest,
            InterfaceError::SendError => Io,
            InterfaceError::Other(_) => Other,
            InterfaceError::IoTodo(_) => Other,
            InterfaceError::Closed => unreachable!(),
        }
    }
}

impl From<ConductorApiError> for AdminInterfaceErrorKind {
    fn from(e: ConductorApiError) -> Self {
        use AdminInterfaceErrorKind::*;
        match e {
            ConductorApiError::CellMissing(_) => Cell,
            ConductorApiError::ConductorError(_) => Conductor,
            ConductorApiError::ZomeInvocationCellMismatch { .. } => Conductor,
            ConductorApiError::Todo(_) => Other,
            ConductorApiError::Io(_) => Io,
            ConductorApiError::SerializationError(_) => Serialization,
        }
    }
}
