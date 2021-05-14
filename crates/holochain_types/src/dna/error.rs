//! Holochain DnaError type.

use holo_hash::{DnaHash, WasmHash};
use holochain_zome_types::zome::ZomeName;
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

    /// DNA not found in a DnaStore
    #[error("The DNA of the following hash was not found in the store: {0}")]
    DnaMissing(DnaHash),

    /// TraitNotFound
    #[error("Trait not found: {0}")]
    TraitNotFound(String),

    /// ZomeFunctionNotFound
    #[error("Zome function not found: {0}")]
    ZomeFunctionNotFound(String),

    /// MrBundleError
    #[error(transparent)]
    MrBundleError(#[from] mr_bundle::error::MrBundleError),

    /// SerializedBytesError
    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),

    /// std::io::Error
    /// we don't #[from] the std::io::Error directly because it doesn't implement Clone
    #[error("std::io::Error: {0}")]
    StdIoError(String),

    /// InvalidWasmHash
    #[error("InvalidWasmHash")]
    InvalidWasmHash,

    /// NonWasmZome
    #[error("Accessed a zome expecting to find a WasmZome, but found other type. Zome name: {0}")]
    NonWasmZome(ZomeName),

    /// DnaHashMismatch
    #[error("DNA file hash mismatch.\nExpected: {0}\nActual: {1}")]
    DnaHashMismatch(DnaHash, DnaHash),

    /// WasmHashMismatch
    #[error("Wasm hash mismatch.\nExpected: {0}\nActual: {1}")]
    WasmHashMismatch(WasmHash, WasmHash),

    /// DnaFileToBundleConversionError
    #[error("Error converting DnaFile to DnaBundle: {0}")]
    DnaFileToBundleConversionError(String),
}

impl From<std::io::Error> for DnaError {
    fn from(error: std::io::Error) -> Self {
        Self::StdIoError(error.to_string())
    }
}

/// Result type for DnaError
pub type DnaResult<T> = Result<T, DnaError>;
