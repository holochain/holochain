use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppManifestError {
    #[error("Missing required field in app manifest: {0}")]
    MissingField(String),
}

pub type AppManifestResult<T> = Result<T, AppManifestError>;
