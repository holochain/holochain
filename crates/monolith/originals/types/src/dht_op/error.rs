use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::header::conversions::WrongHeaderError;
use holochain_zome_types::Header;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum DhtOpError {
    #[error("Tried to create a DhtOp from a Element that requires an Entry. Header type {0:?}")]
    HeaderWithoutEntry(Header),
    #[error(transparent)]
    SerializedBytesError(#[from] SerializedBytesError),
    #[error(transparent)]
    WrongHeaderError(#[from] WrongHeaderError),
}

pub type DhtOpResult<T> = Result<T, DhtOpError>;
