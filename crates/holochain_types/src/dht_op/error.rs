#![allow(missing_docs)]

use super::ChainOpType;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::action::conversions::WrongActionError;
use holochain_zome_types::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DhtOpError {
    #[error(
        "Tried to create a DhtOp from a Record that requires an Entry. Action type {:?}", .0
    )]
    ActionWithoutEntry(Box<Action>),
    #[error(transparent)]
    SerializedBytesError(#[from] SerializedBytesError),
    #[error(transparent)]
    WrongActionError(#[from] WrongActionError),
    #[error("Tried to create DhtOp type {0} with action type {1}")]
    OpActionMismatch(ChainOpType, ActionType),
    #[error("Link requests without tags require a tag in the response")]
    LinkKeyTagMissing,
}

pub type DhtOpResult<T> = Result<T, DhtOpError>;
