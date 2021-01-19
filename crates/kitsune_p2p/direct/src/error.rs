//! Kd Error types

use crate::*;

/// Error related to remote communication.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KdError {
    /// OldGhostError.
    #[error(transparent)]
    OldGhostError(#[from] old_ghost_actor::GhostError),

    /// GhostError.
    #[error(transparent)]
    GhostError(#[from] ghost_actor::GhostError),

    /// Unspecified error.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl KdError {
    /// promote a custom error type to a KdError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }
}

impl From<KitsuneP2pError> for KdError {
    fn from(e: KitsuneP2pError) -> Self {
        KdError::other(e)
    }
}

impl From<KdError> for KitsuneP2pError {
    fn from(e: KdError) -> Self {
        KitsuneP2pError::other(e)
    }
}

impl From<String> for KdError {
    fn from(s: String) -> Self {
        #[derive(Debug, thiserror::Error)]
        struct OtherError(String);
        impl std::fmt::Display for OtherError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        KdError::other(OtherError(s))
    }
}

impl From<&str> for KdError {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl From<KdError> for () {
    fn from(_: KdError) {}
}

impl From<std::io::Error> for KdError {
    fn from(e: std::io::Error) -> Self {
        Self::other(e)
    }
}

impl From<sodoken::SodokenError> for KdError {
    fn from(e: sodoken::SodokenError) -> Self {
        Self::other(e)
    }
}

impl From<serde_json::Error> for KdError {
    fn from(e: serde_json::Error) -> Self {
        Self::other(e)
    }
}

impl From<rusqlite::Error> for KdError {
    fn from(e: rusqlite::Error) -> Self {
        Self::other(e)
    }
}

/// Result type for remote communication.

pub type KdResult<T> = Result<T, KdError>;
