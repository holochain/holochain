use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::header::conversions::WrongHeaderError;
use holochain_zome_types::Header;
use holochain_zome_types::HeaderType;
use thiserror::Error;

use super::DhtOpType;

#[derive(PartialEq, Eq, Clone, Debug, Error)]
pub enum DhtOpError {
    #[error("Tried to create a DhtOp from a Element that requires an Entry. Header type {0:?}")]
    HeaderWithoutEntry(Header),
    #[error(transparent)]
    SerializedBytesError(#[from] SerializedBytesError),
    #[error(transparent)]
    WrongHeaderError(#[from] WrongHeaderError),
    #[error("Tried to create DhtOp type {0} with header type {1}")]
    OpHeaderMismatch(DhtOpType, HeaderType),
}

pub type DhtOpResult<T> = Result<T, DhtOpError>;
