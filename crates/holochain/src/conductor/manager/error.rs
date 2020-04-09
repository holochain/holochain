use crate::conductor::error::ConductorError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ManagedTaskError {
    #[error(transparent)]
    Conductor(#[from] ConductorError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
}

pub type ManagedTaskResult = Result<(), ManagedTaskError>;
