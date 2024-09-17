use holo_hash::DhtOpHash;
use thiserror::Error;

use crate::mutations::StateMutationError;

#[derive(Error, Debug)]
pub enum EventError {
    #[error(transparent)]
    StateMutationError(#[from] StateMutationError),

    #[error("database row not recorded for op hash: {0:?}")]
    RequisiteEventNotFound(DhtOpHash),
}
