use holochain_conductor_api::ExternalApiWireError;
use std::error::Error;

#[derive(Debug, thiserror::Error)]
pub enum ConductorApiError {
    #[error("Websocket error: {0}")]
    WebsocketError(#[from] holochain_websocket::WebsocketError),
    #[error("External API wire error: {0:?}")]
    ExternalApiWireError(ExternalApiWireError),
    #[error("Fresh nonce error: {0}")]
    FreshNonceError(Box<dyn Error + Sync + Send>),
    #[error("Unable to sign zome call: {0}")]
    SignZomeCallError(String),
    #[error("Cell not found")]
    CellNotFound,
    #[error("App not found")]
    AppNotFound,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type ConductorApiResult<T> = Result<T, ConductorApiError>;
