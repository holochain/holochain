#![allow(missing_docs)]

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

    #[error(transparent)]
    Recv(#[from] tokio::sync::broadcast::error::RecvError),
}

pub type ManagedTaskResult = Result<(), ManagedTaskError>;
