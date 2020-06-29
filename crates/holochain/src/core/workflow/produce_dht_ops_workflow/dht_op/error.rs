use crate::core::SourceChainError;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_state::error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DhtOpConvertError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),
    #[error(transparent)]
    SerializedBytesError(#[from] SerializedBytesError),
    #[error(
        "The entry could not be found for a RegisterReplacedBy that has an IntendedFor of Entry"
    )]
    MissingEntry,
    #[error("Data for a DhtOp was missing from the source chain")]
    MissingData,
    #[error("Tried to create a StoreEntry with a header that is not EntryCreate or ElementUpdate")]
    HeaderEntryMismatch,
    #[error(
        "Entry was missing for StoreEntry when private. Maybe the database doesn't have access"
    )]
    StoreEntryOnPrivate,
    #[error("The Header: {0} is the wrong type for this DhtOp: {1}")]
    HeaderMismatch(String, String),
    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
}

pub type DhtOpConvertResult<T> = Result<T, DhtOpConvertError>;
