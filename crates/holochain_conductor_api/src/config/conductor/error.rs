use std::path::PathBuf;
use thiserror::Error;

/// Custom result type with [`ConductorConfigError`] as the error type.
pub type ConductorConfigResult<T> = Result<T, ConductorConfigError>;

/// Custom error type for conductor configuration errors.
#[derive(Error, Debug)]
pub enum ConductorConfigError {
    /// No conductor configuration was found at the specified path.
    #[error("No conductor config found at this path: {0}")]
    ConfigMissing(PathBuf),

    /// The conductor configuration file is not valid YAML.
    #[error("Config deserialization error: {0}")]
    SerializationError(#[from] serde_yaml::Error),

    /// I/O error while working with conductor configuration.
    #[error("Error while performing IO for the Conductor: {0}")]
    IoError(#[from] std::io::Error),

    /// The network configuration is invalid.
    #[error("Invalid network config: {0}")]
    InvalidNetworkConfig(String),
}
