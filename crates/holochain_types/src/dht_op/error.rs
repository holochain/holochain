use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::action::conversions::WrongActionError;
use holochain_zome_types::Action;
use holochain_zome_types::ActionType;
use thiserror::Error;

use super::DhtOpType;

#[derive(PartialEq, Eq, Clone, Debug, Error)]
pub enum DhtOpError {
    #[error("Tried to create a DhtOp from a Element that requires an Entry. Action type {0:?}")]
    ActionWithoutEntry(Action),
    #[error(transparent)]
    SerializedBytesError(#[from] SerializedBytesError),
    #[error(transparent)]
    WrongActionError(#[from] WrongActionError),
    #[error("Tried to create DhtOp type {0} with action type {1}")]
    OpActionMismatch(DhtOpType, ActionType),
    #[error("Link requests without tags require a tag in the response")]
    LinkKeyTagMissing,
}

pub type DhtOpResult<T> = Result<T, DhtOpError>;
