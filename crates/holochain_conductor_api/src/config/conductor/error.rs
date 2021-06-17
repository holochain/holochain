use std::path::PathBuf;
use thiserror::Error;

pub type ConductorConfigResult<T> = Result<T, ConductorConfigError>;

#[derive(Error, Debug)]
pub enum ConductorConfigError {
    #[error("No conductor config found at this path: {0}")]
    ConfigMissing(PathBuf),

    #[error("Config deserialization error: {0}")]
    SerializationError(#[from] serde_yaml::Error),

    #[error("Error while performing IO for the Conductor: {0}")]
    IoError(#[from] std::io::Error),
}
