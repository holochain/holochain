use thiserror::Error;

use crate::mutations::StateMutationError;

#[derive(Error, Debug)]
pub enum EventError {
    #[error(transparent)]
    StateMutationError(#[from] StateMutationError),

    #[error("requisite event not found")]
    RequisiteEventNotFound,
}
