use crate::conductor::api::error::ConductorApiError;
use crate::conductor::entry_def_store::error::EntryDefStoreError;
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::workflow::error::WorkflowError;
use crate::core::workflow::produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertError;
use crate::core::SourceChainError;
use holochain_cascade::error::CascadeError;
use holochain_lmdb::error::DatabaseError;
use holochain_p2p::HolochainP2pError;
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
    #[error("The Dna was not found in the store")]
    DnaMissing,
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
    #[error(transparent)]
    HolochainP2pError(#[from] HolochainP2pError),
    #[error(transparent)]
    ConductorApiError(#[from] Box<ConductorApiError>),
    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),
    #[error(transparent)]
    DhtOpConvertError(#[from] DhtOpConvertError),
    #[error("Todo")]
    Todo,
}

pub type CellResult<T> = Result<T, CellError>;
