use crate::Header;
use thiserror::Error;
use holochain_serialized_bytes::SerializedBytesError;

#[derive(Clone, Debug, Error)]
pub enum DhtOpError {
    #[error(
        "Tried to create a DhtOp from a ChainElement that requires an Entry. Header type {0:?}"
    )]
    HeaderWithoutEntry(Header),
    #[error(transparent)]
    SerializedBytesError(#[from] SerializedBytesError),
}

pub type DhtOpResult<T> = Result<T, DhtOpError>;
