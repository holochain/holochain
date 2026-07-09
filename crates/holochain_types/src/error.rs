//! Error types for DHT op rendering and construction.

#![allow(missing_docs)]

use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::action::conversions::WrongActionError;
use holochain_zome_types::op::ChainOpType;
use holochain_zome_types::prelude::*;
use thiserror::Error;

// The `ActionWithoutEntry`/`OpActionMismatch` diagnostics carry the legacy
// per-variant `Action`/`ActionType`. They are error-message payloads only (never
// used to decide an outcome); the surviving `app_validation_workflow` projects
// its v2 action to this legacy shape via `to_legacy_signed_action` when raising
// them. Shadows the v2 `Action` re-exported by `prelude::*`.
use holochain_zome_types::dependencies::holochain_integrity_types::action::Action;

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
    #[error("Op type {0} does not match the action it was rendered from")]
    OpTypeActionMismatch(ChainOpType),
    #[error("Link requests without tags require a tag in the response")]
    LinkKeyTagMissing,
}

pub type DhtOpResult<T> = Result<T, DhtOpError>;
