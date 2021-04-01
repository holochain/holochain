use holo_hash::{AnyDhtHash, HeaderHash};
use holochain_p2p::HolochainP2pError;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_sqlite::error::DatabaseError;
use holochain_state::source_chain::SourceChainError;
use holochain_types::prelude::*;
use holochain_zome_types::header::conversions::WrongHeaderError;
// use holochain::conductor::CellError;
// use holochain::core::workflow::produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertError;
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Error, Debug)]
pub enum CascadeError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    ElementGroupError(#[from] ElementGroupError),

    #[error(transparent)]
    HeaderError(#[from] HeaderError),

    #[error("Expected this Header to contain an Entry: {0}")]
    EntryMissing(HeaderHash),

    #[error(transparent)]
    DhtOpError(#[from] DhtOpError),

    #[error("Got an invalid response from an authority for the request hash: {0:?}")]
    InvalidResponse(AnyDhtHash),

    #[error(transparent)]
    JoinError(#[from] JoinError),

    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),

    #[error(transparent)]
    NetworkError(#[from] HolochainP2pError),

    #[error(transparent)]
    SerializedBytesError(#[from] SerializedBytesError),

    #[error(transparent)]
    WrongHeaderError(#[from] WrongHeaderError),

    #[error("Cell is an authority for is missing or incorrect: {0}")]
    AuthorityDataError(#[from] AuthorityDataError),

    #[error(transparent)]
    QueryError(#[from] holochain_state::query::StateQueryError),
}

pub type CascadeResult<T> = Result<T, CascadeError>;

#[derive(Error, Debug)]
pub enum AuthorityDataError {
    // #[error(transparent)]
    // DhtOpConvertError(#[from] DhtOpConvertError),
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
    pub fn missing_data<T: std::fmt::Debug>(data: T) -> CascadeError {
        Self::MissingData(format!("Missing header {:?}", data)).into()
    }
    pub fn missing_data_entry<T: std::fmt::Debug>(data: T) -> CascadeError {
        Self::MissingData(format!("Missing entry for header {:?}", data)).into()
    }
    pub fn missing_metadata<T: std::fmt::Debug>(data: T) -> CascadeError {
        Self::MissingMetadata(format!("{:?}", data)).into()
    }
}
