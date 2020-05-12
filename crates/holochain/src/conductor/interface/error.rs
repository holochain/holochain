use crate::conductor::error::ConductorError;
use holochain_serialized_bytes::SerializedBytesError;

/// Interface Error Type
#[derive(Debug, thiserror::Error)]
pub enum InterfaceError {
    #[error(transparent)]
    SerializedBytes(#[from] SerializedBytesError),
    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),
    #[error(transparent)]
    SignalReceive(tokio::sync::broadcast::RecvError),
    #[error(transparent)]
    RequestHandler(ConductorError),
    #[error("Got an unexpected message: {0}")]
    UnexpectedMessage(String),
    #[error("Failed to send across interface")]
    SendError,
    #[error("Other error: {0}")]
    Other(String),
    #[error("Interface closed")]
    Closed,
    // FIXME: update error types in holochain_websocket to use a more specific
    // type than io::Error
    #[error(transparent)]
    IoTodo(#[from] std::io::Error),
    #[error("Failed to find free port")]
    PortError,
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
