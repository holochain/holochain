//! Custom error types for the mr_bundle crate

use crate::manifest::ResourceIdentifier;

/// Any error which can occur in this crate
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MrBundleError {
    /// An IO error
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    /// A manifest references resources that were not provided when attempting to create a bundle.
    #[error("Manifest references resources that were not provided when attempting to create a bundle: {0:?}")]
    MissingResources(Vec<ResourceIdentifier>),

    /// Resources were provided when attempting to create a bundle that were not referenced in the manifest.
    #[error("Resources were provided when attempting to create a bundle that were not referenced in the manifest: {0:?}")]
    UnusedResources(Vec<ResourceIdentifier>),

    /// A messagepack encoding error
    #[error(transparent)]
    MsgpackEncodeError(#[from] rmp_serde::encode::Error),

    /// A messagepack decoding error
    #[error("Failed to decode bundle to [{0}] due to a deserialization error: {1}")]
    MsgpackDecodeError(String, rmp_serde::decode::Error),

    /// A YAML error
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    #[error(transparent)]
    YamlError(#[from] serde_yaml::Error),

    /// A path with no parent directory
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    #[error("The supplied path '{0}' has no parent directory.")]
    ParentlessPath(std::path::PathBuf),

    /// A target directory already exists
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    #[error("The target directory '{0}' already exists.")]
    DirectoryExists(std::path::PathBuf),
}

/// A custom result that uses [`MrBundleError`] as the error type.
pub type MrBundleResult<T> = Result<T, MrBundleError>;
