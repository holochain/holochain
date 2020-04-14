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

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AdminInterfaceError {
    Serialization,
    Cell,
    Conductor,
    Io,
    Runtime,
    BadRequest,
    Other,
}

impl From<InterfaceError> for AdminInterfaceError {
    fn from(error: InterfaceError) -> Self {
        use AdminInterfaceError::*;
        match error {
            InterfaceError::SerializedBytes(_) => Serialization,
            InterfaceError::JoinError(_) => Runtime,
            InterfaceError::SignalReceive(_) => Runtime,
            InterfaceError::RequestHandler(_) => Conductor,
            InterfaceError::UnexpectedMessage(_) => BadRequest,
            InterfaceError::SendError => Io,
            InterfaceError::Other(_) => Other,
            InterfaceError::IoTodo(_) => Other,
        }
    }
}

impl From<ConductorApiError> for AdminInterfaceError {
    fn from(e: ConductorApiError) -> Self {
        use AdminInterfaceError::*;
        match e {
            ConductorApiError::CellMissing(_) => Cell,
            ConductorApiError::ConductorError(_) => Conductor,
            ConductorApiError::Todo(_) => Other,
            ConductorApiError::Io(_) => Io,
            ConductorApiError::SerializationError(_) => Serialization,
        }
    }
}
