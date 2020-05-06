//! Holochain DnaError type.

use thiserror::Error;

/// Holochain DnaError type.
#[derive(Debug, Error)]
pub enum DnaError {
    /// ZomeNotFound
    #[error("Zome not found: {0}")]
    ZomeNotFound(String),

    /// EmptyZome
    #[error("Zome has no code: {0}")]
    EmptyZome(String),

    /// Invalid
    #[error("DNA is invalid: {0}")]
    Invalid(String),

    /// TraitNotFound
    #[error("Trait not found: {0}")]
    TraitNotFound(String),

    /// ZomeFunctionNotFound
    #[error("Zome function not found: {0}")]
    ZomeFunctionNotFound(String),

    /// SerializedBytesError
    #[error("SerializedBytesError: {0}")]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),

    /// std::io::Error
    #[error("std::io::Error: {0}")]
    StdIoError(#[from] std::io::Error),

    /// InvalidWasmHash
    #[error("InvalidWasmHash")]
    InvalidWasmHash,
}
