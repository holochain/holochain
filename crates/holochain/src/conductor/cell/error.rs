use crate::conductor::api::error::ConductorApiError;
use holochain_state::error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CellError {
    #[error("error dealing with workspace state: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("The Dna was not found in the store")]
    DnaMissing,
    #[error("Failed to join the create cell task: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("Genesis failed: {0}")]
    Genesis(#[from] Box<ConductorApiError>),
    #[error("The environment was not created and is missing")]
    EnvMissing,
}

pub type CellResult<T> = Result<T, CellError>;
