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
        "The entry could not be found for a RegisterReplacedBy that has an UpdateBasis of Entry"
    )]
    MissingEntry,
}

pub type DhtOpConvertResult<T> = Result<T, DhtOpConvertError>;
