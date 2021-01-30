use std::path::PathBuf;

use crate::io_error::IoError;

#[derive(Debug, thiserror::Error)]
pub enum MrBundleError {
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    #[error(transparent)]
    BundleError(#[from] BundleError),

    #[cfg(feature = "packing")]
    #[error(transparent)]
    UnpackingError(#[from] UnpackingError),

    #[cfg(feature = "packing")]
    #[error(transparent)]
    PackingError(#[from] PackingError),

    #[error("IO error: {0}")]
    IoError(#[from] IoError),

    #[error(transparent)]
    HttpError(#[from] reqwest::Error),

    #[error(transparent)]
    MsgpackEncodeError(#[from] rmp_serde::encode::Error),

    #[error(transparent)]
    MsgpackDecodeError(#[from] rmp_serde::decode::Error),
}
pub type MrBundleResult<T> = Result<T, MrBundleError>;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum BundleError {
    #[error(
        "The bundled resource path '{0}' is not mentioned in the manifest.
        Make sure that Manifest::location returns this path as a Location::Bundled."
    )]
    BundledPathNotInManifest(std::path::PathBuf),

    #[error("Attempted to resolve a bundled resource not present in this bundle: {0}")]
    BundledResourceMissing(std::path::PathBuf),

    #[error(
        "Cannot use relative paths for local locations. The following local path is relative: {0}"
    )]
    RelativeLocalPath(std::path::PathBuf),
}
pub type BundleResult<T> = Result<T, BundleError>;

#[cfg(feature = "packing")]
#[derive(Debug, thiserror::Error)]
pub enum UnpackingError {
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    #[error("IO error: {0}")]
    IoError(#[from] IoError),

    #[error(transparent)]
    YamlError(#[from] serde_yaml::Error),

    #[error("The supplied path '{0}' has no parent directory.")]
    ParentlessPath(std::path::PathBuf),

    #[error("The target directory '{0}' already exists.")]
    DirectoryExists(std::path::PathBuf),

    #[error("When imploding a bundle directory, the absolute manifest path specified did not match the relative path expected by the manifest.
    Absolute path: '{0}'. Relative path: '{1}'.")]
    ManifestPathSuffixMismatch(std::path::PathBuf, std::path::PathBuf),
}
#[cfg(feature = "packing")]
pub type UnpackingResult<T> = Result<T, UnpackingError>;

#[cfg(feature = "packing")]
#[derive(Debug, thiserror::Error)]
pub enum PackingError {
    #[error("Must supply the path to the manifest file inside a bundle directory to pack. You supplied: {0}. Original error: {1}")]
    BadManifestPath(std::path::PathBuf, std::io::Error),
}
#[cfg(feature = "packing")]
pub type PackingResult<T> = Result<T, PackingError>;
