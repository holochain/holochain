use sx_types::error::SkunkError;
use sx_types::prelude::*;
use thiserror::Error;
use holochain_json_api::error::JsonError;

#[derive(Error, Debug, PartialEq)]
pub enum SourceChainError {
    #[error("The source chain is empty: it needs to be initialized before using")]
    ChainEmpty,

    #[error("The source chain's structure is invalid. This error is not recoverable. Detail:\n{0}")]
    InvalidStructure(ChainInvalidReason),

    #[error("The source chain's head is pointing to an address which has no content.")]
    MissingHead,

    #[error("The content at address {0} is malformed and can't be deserialized.")]
    MalformedEntry(Address),

    #[error("Persistence error: {0}")]
    PersistenceError(#[from] PersistenceError),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] JsonError),

    #[error(transparent)]
    Generic(#[from] SkunkError),
}

#[derive(Error, Debug, PartialEq)]
pub enum ChainInvalidReason {
    #[error("A valid chain always begins with a Dna entry, followed by an Agent entry.")]
    MissingGenesis,
}

pub type SourceChainResult<T> = Result<T, SourceChainError>;
