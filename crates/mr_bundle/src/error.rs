//! Custom error types for the mr_bundle crate

use crate::manifest::ResourceIdentifier;

/// Any error which can occur in this crate
#[derive(Debug, thiserror::Error)]
pub enum MrBundleError {
    /// An IO error
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    /// A bundle error
    #[error(transparent)]
    BundleError(#[from] BundleError),

    /// An unpacking error
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    #[error(transparent)]
    UnpackingError(#[from] UnpackingError),

    /// A fs error
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    #[error(transparent)]
    PackingError(#[from] PackingError),

    /// A messagepack encoding error
    #[error(transparent)]
    MsgpackEncodeError(#[from] rmp_serde::encode::Error),

    /// A messagepack decoding error
    #[error("Failed to decode bundle to [{0}] due to a deserialization error: {1}")]
    MsgpackDecodeError(String, rmp_serde::decode::Error),

    /// A bundle validation error
    #[error("This bundle failed to validate because: {0}")]
    BundleValidationError(String),
}

/// A custom result that uses [`MrBundleError`] as the error type.
pub type MrBundleResult<T> = Result<T, MrBundleError>;

/// Errors which can occur while constructing a Bundle
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum BundleError {
    /// A manifest references resources that were not provided when attempting to create a bundle.
    #[error("Manifest references resources that were not provided when attempting to create a bundle: {0:?}")]
    MissingResources(Vec<ResourceIdentifier>),

    /// Resources were provided when attempting to create a bundle that were not referenced in the manifest.
    #[error("Resources were provided when attempting to create a bundle that were not referenced in the manifest: {0:?}")]
    UnusedResources(Vec<ResourceIdentifier>),

    /// A resource was provided that is not used in the manifest.
    #[error(
        "The bundled resource path '{0}' is not mentioned in the manifest.
        Make sure that Manifest::location returns this path as a Location::Bundled."
    )]
    BundledPathNotInManifest(std::path::PathBuf),

    /// A resource was referenced but not found in the bundle.
    #[error("Attempted to resolve a bundled resource not present in this bundle: {0}")]
    BundledResourceMissing(std::path::PathBuf),

    /// Used a relative local path, which is not supported.
    #[error(
        "Cannot use relative paths for local locations. The following local path is relative: {0}"
    )]
    RelativeLocalPath(std::path::PathBuf),
}

/// A custom result that uses [`BundleError`] as the error type.
pub type BundleResult<T> = Result<T, BundleError>;

/// Errors which can occur while unpacking resources from a Bundle
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
#[derive(Debug, thiserror::Error)]
pub enum UnpackingError {
    /// An IO error
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    /// A YAML error
    #[error(transparent)]
    YamlError(#[from] serde_yaml::Error),

    /// A path with no parent directory
    #[error("The supplied path '{0}' has no parent directory.")]
    ParentlessPath(std::path::PathBuf),

    /// A target directory already exists
    #[error("The target directory '{0}' already exists.")]
    DirectoryExists(std::path::PathBuf),

    /// Invalid path
    #[error("When imploding a bundle directory, the absolute manifest path specified did not match the relative path expected by the manifest.
    Absolute path: '{0}'. Relative path: '{1}'.")]
    ManifestPathSuffixMismatch(std::path::PathBuf, std::path::PathBuf),
}

/// Custom result type with the error type [`UnpackingError`].
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub type UnpackingResult<T> = Result<T, UnpackingError>;

/// Errors which can occur while fs resources into a Bundle
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
#[derive(Debug, thiserror::Error)]
pub enum PackingError {
    /// An invalid manifest path
    #[error("Must supply the path to the manifest file inside a bundle directory to pack. You supplied: {0}. Original error: {1}")]
    BadManifestPath(std::path::PathBuf, std::io::Error),
}

/// A custom result that uses [`PackingError`] as the error type.
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub type PackingResult<T> = Result<T, PackingError>;
