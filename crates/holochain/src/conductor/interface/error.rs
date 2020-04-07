use holochain_serialized_bytes::SerializedBytesError;

/// Interface Error Type
#[derive(Debug, thiserror::Error)]
pub enum InterfaceError {
    SerializedBytes(#[from] SerializedBytesError),
    JoinError(#[from] tokio::task::JoinError),
    SignalReceive(tokio::sync::broadcast::RecvError),
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
