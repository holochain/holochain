use crate::{
    conductor::api::error::ConductorApiError,
    core::{
        ribosome::{error::RibosomeError, guest_callback::init::InitResult},
        workflow::error::WorkflowError,
    },
};
use holochain_state::error::DatabaseError;
use holochain_types::cell::CellId;
use std::path::PathBuf;
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
    #[error("This cell has not had a successful genesis and cannot be created")]
    CellWithoutGenesis(CellId),
    #[error("The cell failed to cleanup its environment because: {0}. Recommend manually deleting the database at: {1}")]
    Cleanup(String, PathBuf),
    #[error(transparent)]
    WorkflowError(#[from] Box<WorkflowError>),
    #[error(transparent)]
    WorkspaceError(#[from] crate::core::state::workspace::WorkspaceError),
    #[error(transparent)]
    RibosomeError(#[from] RibosomeError),
    #[error("The cell tried to run the initialize zomes callback but failed because {0:?}")]
    InitFailed(InitResult),
    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),
}

pub type CellResult<T> = Result<T, CellError>;
