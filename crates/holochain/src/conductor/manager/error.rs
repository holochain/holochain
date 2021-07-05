#![allow(missing_docs)]

use crate::conductor::error::ConductorError;
use thiserror::Error;

/// An error that is thrown from within the Task Manager itself.
/// An unrecoverable ManagedTaskError can be bubbled up into a TaskManagerError.
#[derive(Error, Debug)]
pub enum TaskManagerError {
    #[error("Conductor has exited due to an unrecoverable error in a managed task {0}")]
    Unrecoverable(ManagedTaskError),

    #[error("Task manager failed to start")]
    TaskManagerFailedToStart,

    #[error("Task manager encountered an internal error: {0}")]
    Internal(Box<dyn std::error::Error + Send + Sync>),
}

impl TaskManagerError {
    pub fn internal<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Internal(Box::new(err))
    }
}

pub type TaskManagerResult = Result<(), TaskManagerError>;

/// An error that is thrown from within a managed task
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

impl ManagedTaskError {
    pub fn is_recoverable(&self) -> bool {
        use ConductorError as C;
        use ManagedTaskError::*;
        #[allow(clippy::match_like_matches_macro)]
        match self {
            Io(_) | Join(_) | Recv(_) => false,
            Conductor(err) => match err {
                C::ShuttingDown => true,
                // TODO: identify all recoverable cases
                _ => false,
            },
        }
    }
}
