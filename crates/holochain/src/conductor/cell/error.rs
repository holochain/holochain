use crate::{
    conductor::api::error::ConductorApiError,
    conductor::entry_def_store::error::EntryDefStoreError,
    core::{
        ribosome::{error::RibosomeError, guest_callback::init::InitResult},
        state::cascade::error::CascadeError,
        workflow::{
            error::WorkflowError, produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertError,
        },
        SourceChainError,
    },
};
use holochain_p2p::HolochainP2pError;
use holochain_state::error::DatabaseError;
use holochain_types::{cell::CellId, header::error::HeaderError};
use holochain_zome_types::header::conversions::WrongHeaderError;
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
    Genesis(#[from] Box<ConductorApiError>),
    #[error(transparent)]
    HeaderError(#[from] HeaderError),
    #[error("This cell has not had a successful genesis and cannot be created")]
    CellWithoutGenesis(CellId),
    #[error("The cell failed to cleanup its environment because: {0}. Recommend manually deleting the database at: {1}")]
    Cleanup(String, PathBuf),
    #[error(transparent)]
    EntryDefStoreError(#[from] EntryDefStoreError),
    #[error(transparent)]
    WorkflowError(#[from] Box<WorkflowError>),
    #[error(transparent)]
    WorkspaceError(#[from] crate::core::state::workspace::WorkspaceError),
    #[error(transparent)]
    RibosomeError(#[from] RibosomeError),
    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
    #[error("The cell tried to run the initialize zomes callback but failed because {0:?}")]
    InitFailed(InitResult),
    #[error(transparent)]
    HolochainP2pError(#[from] HolochainP2pError),
    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),
    #[error(transparent)]
    DhtOpConvertError(#[from] DhtOpConvertError),
    #[error("Cell is an authority for is missing or incorrect: {0}")]
    AuthorityDataError(#[from] AuthorityDataError),
    #[error("Todo")]
    Todo,
}

pub type CellResult<T> = Result<T, CellError>;

#[derive(Error, Debug)]
pub enum AuthorityDataError {
    #[error(transparent)]
    DhtOpConvertError(#[from] DhtOpConvertError),
    #[error(transparent)]
    WrongHeaderError(#[from] WrongHeaderError),
    #[error(transparent)]
    HeaderError(#[from] HeaderError),
    #[error("Missing element data: {0:?}")]
    MissingData(String),
    #[error("Missing metadata: {0:?}")]
    MissingMetadata(String),
}

impl AuthorityDataError {
    pub fn missing_data<T: std::fmt::Debug>(data: T) -> CellError {
        Self::MissingData(format!("Missing header {:?}", data)).into()
    }
    pub fn missing_data_entry<T: std::fmt::Debug>(data: T) -> CellError {
        Self::MissingData(format!("Missing entry for header {:?}", data)).into()
    }
    pub fn missing_metadata<T: std::fmt::Debug>(data: T) -> CellError {
        Self::MissingMetadata(format!("{:?}", data)).into()
    }
}
