#[derive(Debug, thiserror::Error)]
pub enum MrBundleError {
    #[error(transparent)]
    BundleError(#[from] BundleError),

    #[cfg(feature = "exploding")]
    #[error(transparent)]
    ExplodeError(#[from] ExplodeError),

    #[cfg(feature = "exploding")]
    #[error(transparent)]
    ImplodeError(#[from] ImplodeError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

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
}
pub type BundleResult<T> = Result<T, BundleError>;

#[cfg(feature = "exploding")]
#[derive(Debug, thiserror::Error)]
pub enum ExplodeError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    YamlError(#[from] serde_yaml::Error),

    #[error("The supplied path '{0}' has no parent directory.")]
    ParentlessPath(std::path::PathBuf),

    #[error("When imploding a bundle directory, the absolute manifest path specified did not match the relative path expected by the manifest.
    Absolute path: '{0}'. Relative path: '{1}'.")]
    ManifestPathSuffixMismatch(std::path::PathBuf, std::path::PathBuf),
}
#[cfg(feature = "exploding")]
pub type ExplodeResult<T> = Result<T, ExplodeError>;

#[cfg(feature = "exploding")]
#[derive(Debug, thiserror::Error)]
pub enum ImplodeError {
    #[error("Must supply the path to the manifest file inside a bundle directory to implode. You supplied: {0}. Original error: {1}")]
    BadManifestPath(std::path::PathBuf, std::io::Error),
}
#[cfg(feature = "exploding")]
pub type ImplodeResult<T> = Result<T, ImplodeError>;
