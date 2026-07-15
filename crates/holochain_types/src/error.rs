//! Error types for DHT op rendering and construction.

#![allow(missing_docs)]

use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::action::conversions::WrongActionError;
use holochain_zome_types::action::{Action, ActionType};
use holochain_zome_types::op::ChainOpType;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DhtOpError {
    // These diagnostics carry the `Action`/`ActionType` as error-message
    // payloads only (never used to decide an outcome).
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
    #[error("Op type {0} does not match the action it was rendered from")]
    OpTypeActionMismatch(ChainOpType),
    #[error("Link requests without tags require a tag in the response")]
    LinkKeyTagMissing,
}

pub type DhtOpResult<T> = Result<T, DhtOpError>;
