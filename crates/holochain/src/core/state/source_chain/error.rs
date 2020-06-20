use holochain_serialized_bytes::prelude::*;
use holochain_state::error::DatabaseError;
use holochain_types::{
    composite_hash::{EntryHash, HeaderAddress},
    dht_op::error::DhtOpError,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SourceChainError {
    #[error("The source chain is empty, but is expected to have been initialized")]
    ChainEmpty,

    #[error("Attempted to commit a bundle to the source chain, but the source chain head has moved since the bundle began. Bundle head: {0:?}, Current head: {1:?}")]
    HeadMoved(Option<HeaderAddress>, Option<HeaderAddress>),

    #[error(
        "The source chain's structure is invalid. This error is not recoverable. Detail:\n{0}"
    )]
    InvalidStructure(ChainInvalidReason),

    #[error("The source chain's head is pointing to an address which has no content.")]
    MissingHead,

    #[error("The content at address {0} is malformed and can't be deserialized.")]
    MalformedEntry(EntryHash),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] SerializedBytesError),

    #[error("Workspace error: {0}")]
    DatabaseError(#[from] DatabaseError),

    #[error("SerdeJson Error: {0}")]
    SerdeJsonError(String),

    /// Element signature doesn't validate against the header
    #[error("Element signature is invalid")]
    InvalidSignature,

    /// Element previous header reference is invalid
    #[error("Element previous header reference is invalid: {0}")]
    InvalidPreviousHeader(String),

    #[error("KeystoreError: {0}")]
    KeystoreError(#[from] holochain_keystore::KeystoreError),

    #[error(transparent)]
    BlockOnError(#[from] tokio_safe_block_on::BlockOnError),

    #[error(transparent)]
    DhtOpError(#[from] DhtOpError),

    #[error("Required the scratch space to be empty but contained values")]
    ScratchNotFresh,
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
    HeaderAndEntryMismatch(EntryHash),

    #[error("Content was expected to definitely exist at this address, but didn't: {0}")]
    MissingData(EntryHash),
}

pub type SourceChainResult<T> = Result<T, SourceChainError>;
