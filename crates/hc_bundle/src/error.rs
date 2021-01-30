use std::path::PathBuf;

use holochain_serialized_bytes::SerializedBytesError;

/// HcBundleError type.
#[derive(Debug, thiserror::Error)]
pub enum HcBundleError {
    /// std::io::Error
    #[error("IO error: {0}")]
    StdIoError(#[from] std::io::Error),

    /// Missing filesystem path
    #[error("Couldn't find path: {1:?}. Detail: {0}")]
    PathNotFound(std::io::Error, PathBuf),

    /// DnaError
    #[error("DNA error: {0}")]
    DnaError(#[from] holochain_types::dna::DnaError),

    /// MrBundleError
    #[error(transparent)]
    MrBundleError(#[from] mr_bundle::error::MrBundleError),

    /// SerializedBytesError
    #[error("Internal serialization error: {0}")]
    SerializedBytesError(#[from] SerializedBytesError),

    /// serde_yaml::Error
    #[error("YAML serialization error: {0}")]
    SerdeYamlError(#[from] serde_yaml::Error),

    /// InvalidInput
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// anything else
    #[error("Unknown error: {0}")]
    MiscError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error(
        "This file should have a '{}' extension: {0}",
        crate::dna::DNA_BUNDLE_EXT
    )]
    FileExtensionMissing(PathBuf),
}

/// HcBundle Result type.
pub type HcBundleResult<T> = Result<T, HcBundleError>;
