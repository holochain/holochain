use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::action::conversions::WrongActionError;
use holochain_zome_types::Action;
use holochain_zome_types::ActionType;
use thiserror::Error;

use super::DhtOpType;

#[derive(Debug, Error)]
pub enum DhtOpError {
    #[error(
        "Tried to create a DhtOp from a Record that requires an Entry. Action type {:?}. Backtrace: {:?}", .0, .1
    )]
    ActionWithoutEntry(Action, std::sync::Arc<std::backtrace::Backtrace>),
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

pub fn backtrace() -> std::sync::Arc<std::backtrace::Backtrace> {
    std::sync::Arc::new(std::backtrace::Backtrace::capture())
}
