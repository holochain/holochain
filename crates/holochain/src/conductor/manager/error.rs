#![allow(missing_docs)]

use crate::conductor::error::ConductorError;
use thiserror::Error;

#[allow(clippy::large_enum_variant)]
#[derive(Error, Debug)]
pub enum ShutdownError {
    #[error("Conductor has exited due to an unrecoverable error {0}")]
    Unrecoverable(ManagedTaskError),
    #[error("Task manager failed to start")]
    TaskManagerFailedToStart,
}
#[derive(Error, Debug)]
pub enum ManagedTaskError {
    #[error(transparent)]
    Conductor(#[from] ConductorError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),

    #[error(transparent)]
    Recv(#[from] tokio::sync::broadcast::error::RecvError),
}

pub type ManagedTaskResult = Result<(), ManagedTaskError>;

pub type ShutdownResult = Result<(), ShutdownError>;
