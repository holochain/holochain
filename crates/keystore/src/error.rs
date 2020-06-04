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

    /// Used by dependents to specify an invalid signature of some data
    #[error("Invalid signature {0:?}, for {1}")]
    InvalidSignature(Signature, String),

    /// Unexpected Internal Error.
    #[error("Other: {0}")]
    Other(String),
}

impl std::cmp::PartialEq for KeystoreError {
    fn eq(&self, o: &Self) -> bool {
        format!("{:?}", self) == format!("{:?}", o)
    }
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
