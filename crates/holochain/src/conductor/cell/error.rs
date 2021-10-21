use super::INIT_MUTEX_TIMEOUT_SECS;
use crate::conductor::entry_def_store::error::EntryDefStoreError;
use crate::conductor::{api::error::ConductorApiError, error::ConductorError};
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::workflow::error::WorkflowError;
use crate::core::SourceChainError;
use holochain_cascade::error::CascadeError;
use holochain_p2p::HolochainP2pError;
use holochain_sqlite::error::DatabaseError;
use holochain_types::prelude::*;
use holochain_zome_types::cell::CellId;

use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CellError {
    #[error("error dealing with workspace state: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error(transparent)]
    CascadeError(#[from] CascadeError),
    #[error("Failed to join the create cell task: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("Genesis failed: {0}")]
    Genesis(Box<ConductorApiError>),
    #[error(transparent)]
    HeaderError(#[from] HeaderError),
    #[error("This cell has not had a successful genesis and cannot be created")]
    CellWithoutGenesis(CellId),
    #[error(
        "The cell failed to cleanup its environment because: {0}. Recommend manually deleting the database at: {1}"
    )]
    Cleanup(String, PathBuf),
    #[error(transparent)]
    DnaError(#[from] DnaError),
    #[error(transparent)]
    EntryDefStoreError(#[from] EntryDefStoreError),
    #[error(transparent)]
    WorkflowError(#[from] Box<WorkflowError>),
    #[error(transparent)]
    WorkspaceError(#[from] holochain_state::workspace::WorkspaceError),
    #[error(transparent)]
    RibosomeError(#[from] RibosomeError),
    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
    #[error("The cell tried to run the initialize zomes callback but failed because {0:?}")]
    InitFailed(InitResult),
    #[error(
        "Another zome function has triggered the `init()` callback, which has been blocking this zome call for longer than {} seconds. Giving up.",
        INIT_MUTEX_TIMEOUT_SECS
    )]
    InitTimeout,
    #[error("Failed to get or create the cache for this dna {0:?}")]
    FailedToCreateCache(Box<ConductorError>),
    #[error("Failed to get or create the authored db for this dna {0:?}")]
    FailedToCreateAuthoredDb(Box<ConductorError>),
    #[error("Failed to get or create the DHT db for this dna {0:?}")]
    FailedToCreateDhtDb(Box<ConductorError>),
    #[error(transparent)]
    HolochainP2pError(#[from] HolochainP2pError),
    #[error(transparent)]
    ConductorError(#[from] Box<ConductorError>),
    #[error(transparent)]
    ConductorApiError(#[from] Box<ConductorApiError>),
    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),
    #[error("Todo")]
    Todo,
    #[error("The op: {0:?} is missing for this receipt")]
    OpMissingForReceipt(DhtOpHash),
    #[error(transparent)]
    StateQueryError(#[from] holochain_state::query::StateQueryError),
    #[error(transparent)]
    StateMutationError(#[from] holochain_state::mutations::StateMutationError),
}

pub type CellResult<T> = Result<T, CellError>;
