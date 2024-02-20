#[derive(Debug, thiserror::Error)]
pub enum BootstrapClientError {
    /// std::io::Error
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    /// Reqwest crate.
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    /// Bootstrap call failed.
    #[error("Bootstrap Error: {0}")]
    Bootstrap(Box<str>),

    /// Integer casting failed.
    #[error(transparent)]
    TryFromInt(#[from] std::num::TryFromIntError),

    /// SystemTime call failed.
    #[error(transparent)]
    SystemTime(#[from] std::time::SystemTimeError),
}

pub type BootstrapClientResult<T> = Result<T, BootstrapClientError>;
