use ghost_actor::GhostError;
use holochain_serialized_bytes::SerializedBytesError;

#[derive(Debug, thiserror::Error)]
pub enum WebsocketError {
    #[error(transparent)]
    GhostError(#[from] GhostError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Infallible(#[from] std::convert::Infallible),
    #[error(transparent)]
    SerializedBytesError(#[from] SerializedBytesError),
    #[error("Failed to receive response to request")]
    FailedToRecvResp,
    #[error("Failed to send response to request")]
    FailedToSendResp,
    #[error("The websocket connection has shutdown")]
    Shutdown,
}

pub type WebsocketResult<T> = Result<T, WebsocketError>;
