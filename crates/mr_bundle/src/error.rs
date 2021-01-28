#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum BundleError {
    #[error(
        "The bundled resource path '{0}' is not mentioned in the manifest.
        Make sure that Manifest::location returns this path as a Location::Bundled."
    )]
    BundledPathNotInManifest(std::path::PathBuf),
}

pub type BundleResult<T> = Result<T, BundleError>;

#[derive(Debug, thiserror::Error)]
pub enum MrBundleError {
    #[error(transparent)]
    BundleError(#[from] BundleError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    HttpError(#[from] reqwest::Error),

    #[error(transparent)]
    EncodeError(#[from] rmp_serde::encode::Error),

    #[error(transparent)]
    DecodeError(#[from] rmp_serde::decode::Error),
}

pub type MrBundleResult<T> = Result<T, MrBundleError>;
