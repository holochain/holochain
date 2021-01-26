#[derive(Debug, thiserror::Error)]
pub enum MrBundleError {
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
