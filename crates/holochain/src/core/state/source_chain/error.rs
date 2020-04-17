use holochain_serialized_bytes::prelude::*;
use sx_state::error::DatabaseError;
use sx_types::prelude::*;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum SourceChainError {
    #[error("The source chain is empty, but is expected to have been initialized")]
    ChainEmpty,

    #[error("Attempted to commit a bundle to the source chain, but the source chain head has moved since the bundle began. Bundle head: {0:?}, Current head: {1:?}")]
    HeadMoved(Option<Address>, Option<Address>),

    #[error(
        "The source chain's structure is invalid. This error is not recoverable. Detail:\n{0}"
    )]
    InvalidStructure(ChainInvalidReason),

    #[error("The source chain's head is pointing to an address which has no content.")]
    MissingHead,

    #[error("The content at address {0} is malformed and can't be deserialized.")]
    MalformedEntry(Address),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] SerializedBytesError),

    #[error("Workspace error: {0}")]
    DatabaseError(#[from] DatabaseError),

    #[error("SerdeJson Error: {0}")]
    SerdeJsonError(String),
}

// serde_json::Error does not implement PartialEq - why is that a requirement??
impl From<serde_json::Error> for SourceChainError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerdeJsonError(format!("{:?}", e))
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum ChainInvalidReason {
    #[error("A valid chain always begins with a Dna entry, followed by an Agent entry.")]
    GenesisDataMissing,

    #[error("A chain header and its corresponding entry have a discrepancy. Entry address: {0}")]
    HeaderAndEntryMismatch(Address),

    #[error("Content was expected to definitely exist at this address, but didn't: {0}")]
    MissingData(Address),
}

pub type SourceChainResult<T> = Result<T, SourceChainError>;
