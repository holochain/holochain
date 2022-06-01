// use crate::holochain::core::workflow::produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertError;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_p2p::HolochainP2pError;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::error::DatabaseError;
use holochain_types::prelude::*;
use thiserror::Error;

use crate::prelude::StateMutationError;
use crate::query::StateQueryError;
use crate::scratch::ScratchError;
use crate::scratch::SyncScratchError;

#[derive(Error, Debug)]
pub enum SourceChainError {
    #[error("The source chain is empty, but is expected to have been initialized")]
    ChainEmpty,

    #[error(
        "Attempted to commit a bundle to the source chain, but the source chain head has moved since the bundle began. Bundle head: {2:?}, Current head: {3:?}"
    )]
    HeadMoved(
        Vec<(Option<Zome>, SignedHeaderHashed)>,
        Vec<EntryHashed>,
        Option<HeaderHash>,
        Option<(HeaderHash, u32, Timestamp)>,
    ),

    #[error(transparent)]
    TimestampError(#[from] holochain_zome_types::TimestampError),

    #[error(transparent)]
    ScratchError(#[from] ScratchError),

    #[error("Attempted to write anything other than the countersigning session entry while the chain was locked for a countersigning session.")]
    ChainLocked,

    #[error("Attempted to write a countersigning session that has already expired")]
    LockExpired,

    #[error("Attempted to write anything other than the countersigning session entry at the same time as the session entry.")]
    DirtyCounterSigningWrite,

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

    #[error("InvalidCommit error: {0}")]
    InvalidCommit(String),

    #[error("InvalidLink error: {0}")]
    InvalidLink(String),

    #[error("KeystoreError: {0}")]
    KeystoreError(#[from] holochain_keystore::KeystoreError),

    #[error(transparent)]
    DhtOpError(#[from] DhtOpError),

    #[error(transparent)]
    HolochainP2pError(#[from] HolochainP2pError),

    #[error("Required the scratch space to be empty but contained values")]
    ScratchNotFresh,

    /// Element signature doesn't validate against the header
    #[error("Element associated with header {0} was not found on the source chain")]
    ElementMissing(String),

    #[error(transparent)]
    ElementGroupError(#[from] ElementGroupError),

    #[error(transparent)]
    StateMutationError(#[from] StateMutationError),

    #[error(transparent)]
    StateQueryError(#[from] StateQueryError),

    #[error(transparent)]
    SyncScratchError(#[from] SyncScratchError),

    #[error(transparent)]
    CounterSigningError(#[from] CounterSigningError),

    #[error("The source chain was missing for a host call that requires it.")]
    SourceChainMissing,

    #[error("The supplied query parameters contains filters that are mutually incompatible.
             In particular, `sequence_range` cannot currently be used with any other filter.
             In the future, all filters will be compatible with each other and this will not be an error.")]
    UnsupportedQuery(ChainQueryFilter),

    /// Other
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl SourceChainError {
    /// promote a custom error type to a SourceChainError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }
}

// serde_json::Error does not implement PartialEq - why is that a requirement??
impl From<serde_json::Error> for SourceChainError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerdeJsonError(format!("{:?}", e))
    }
}

impl From<one_err::OneErr> for SourceChainError {
    fn from(e: one_err::OneErr) -> Self {
        Self::other(e)
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ChainInvalidReason {
    #[error("A valid chain always begins with a Dna entry, followed by an Agent entry.")]
    GenesisDataMissing,

    #[error("A genesis element contains incorrect data.")]
    MalformedGenesisData,

    #[error("A chain header and its corresponding entry have a discrepancy. Entry address: {0}")]
    HeaderAndEntryMismatch(EntryHash),

    #[error("Content was expected to definitely exist at this address, but didn't: {0}")]
    MissingData(EntryHash),
}

pub type SourceChainResult<T> = Result<T, SourceChainError>;
