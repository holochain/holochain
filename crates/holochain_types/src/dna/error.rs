//! Holochain DnaError type.

use holo_hash::{DnaHash, WasmHash};
use holochain_zome_types::zome::error::ZomeError;
use thiserror::Error;

/// Holochain DnaError type.
#[derive(Debug, Error)]
pub enum DnaError {
    /// EmptyZome
    #[error("Zome has no code: {0}")]
    EmptyZome(String),

    /// Invalid
    #[error("DNA is invalid: {0}")]
    Invalid(String),

    /// DNA not found in a RibosomeStore
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

    /// serde_yaml Error
    #[error(transparent)]
    YamlSerializationError(#[from] serde_yaml::Error),

    /// SerializedBytesError
    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),

    /// From ZomeError
    #[error(transparent)]
    ZomeError(#[from] ZomeError),

    /// std::io::Error
    /// we don't #[from] the std::io::Error directly because it doesn't implement Clone
    #[error("std::io::Error: {0}")]
    StdIoError(String),

    /// InvalidWasmHash
    #[error("InvalidWasmHash")]
    InvalidWasmHash,

    /// DnaHashMismatch
    #[error("DNA file hash mismatch.\nExpected: {0}\nActual: {1}")]
    DnaHashMismatch(DnaHash, DnaHash),

    /// WasmHashMismatch
    #[error("Wasm hash mismatch.\nExpected: {0}\nActual: {1}")]
    WasmHashMismatch(WasmHash, WasmHash),

    /// DnaFileToBundleConversionError
    #[error("Error converting DnaFile to DnaBundle: {0}")]
    DnaFileToBundleConversionError(String),

    #[error("All zome names must be unique within a DNA. Found duplicate: {0}")]
    DuplicateZomeNames(String),

    #[error("Zome dependency {0} for {1} is not pointing at an existing integrity zome that is not itself")]
    DanglingZomeDependency(String, String),
}

impl From<std::io::Error> for DnaError {
    fn from(error: std::io::Error) -> Self {
        Self::StdIoError(error.to_string())
    }
}

/// Result type for DnaError
pub type DnaResult<T> = Result<T, DnaError>;
