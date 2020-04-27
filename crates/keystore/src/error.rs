use crate::*;

/// Keystore Error Type.
#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    /// An error generated from the GhostActor system.
    #[error("GhostError: {0}")]
    GhostError(#[from] ghost_actor::GhostError),

    /// Error serializing data.
    #[error("SerializedBytesError: {0}")]
    SerializedBytesError(#[from] SerializedBytesError),

    /// Holochain Crypto Erro.
    #[error("CryptoError: {0}")]
    CryptoError(#[from] holochain_crypto::CryptoError),

    /// Unexpected Internal Error.
    #[error("Other: {0}")]
    Other(String),
}

impl From<String> for KeystoreError {
    fn from(e: String) -> Self {
        KeystoreError::Other(e)
    }
}

impl From<&String> for KeystoreError {
    fn from(e: &String) -> Self {
        e.to_string().into()
    }
}

impl From<&str> for KeystoreError {
    fn from(e: &str) -> Self {
        e.to_string().into()
    }
}
