use std::path::PathBuf;

use holochain_serialized_bytes::SerializedBytesError;
use holochain_util::ffs;

/// HcBundleError type.
#[derive(Debug, thiserror::Error)]
pub enum HcBundleError {
    /// std::io::Error
    #[error("IO error: {0}")]
    StdIoError(#[from] std::io::Error),

    #[error("ffs::IoError: {0}")]
    FfsIoError(#[from] ffs::IoError),

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

    /// anything else
    #[error("Unknown error: {0}")]
    MiscError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("This file should have a '.{0}' extension: {1}")]
    FileExtensionMissing(&'static str, PathBuf),
}

/// HcBundle Result type.
pub type HcBundleResult<T> = Result<T, HcBundleError>;
