use crate::*;
use holochain_zome_types::signature::Signature;

/// Keystore Error Type.
#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    /// An error generated from the GhostActor system.
    #[error("GhostError: {0}")]
    GhostError(#[from] ghost_actor::GhostError),

    /// Error from Lair
    #[error(transparent)]
    LairError(#[from] lair_keystore_api::LairError),

    /// Error serializing data.
    #[error("SerializedBytesError: {0}")]
    SerializedBytesError(#[from] SerializedBytesError),

    /// Used by dependents to specify an invalid signature of some data
    #[error("Invalid signature {0:?}, for {1}")]
    InvalidSignature(Signature, String),

    /// Used in TryFrom implementations for some zome types.
    #[error("Secure primitive error: {0}")]
    SecurePrimitiveError(#[from] holochain_zome_types::SecurePrimitiveError),

    /// Unexpected Internal Error.
    #[error("Other: {0}")]
    Other(String),
}

impl From<KeystoreError> for lair_keystore_api::LairError {
    fn from(e: KeystoreError) -> lair_keystore_api::LairError {
        match e {
            KeystoreError::LairError(e) => e,
            _ => lair_keystore_api::LairError::other(e),
        }
    }
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
