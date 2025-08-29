use crate::query::StateQueryError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateMutationError {
    #[error(transparent)]
    Sql(#[from] holochain_sqlite::rusqlite::Error),

    #[error(transparent)]
    Infallible(#[from] std::convert::Infallible),

    #[error(transparent)]
    DatabaseError(#[from] holochain_sqlite::error::DatabaseError),

    #[error(transparent)]
    DhtOpError(#[from] holochain_types::dht_op::DhtOpError),

    #[error(transparent)]
    StateQueryError(#[from] StateQueryError),

    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),

    #[error(transparent)]
    ScheduleError(#[from] holochain_zome_types::schedule::ScheduleError),

    #[error("Authors of actions must all be the same when inserting to the source chain")]
    AuthorsMustMatch,

    #[error("Cannot remove a fully published countersigning session")]
    CannotRemoveFullyPublished,

    #[error("The operation was not found in the cache")]
    OpNotFoundInCache,
}

pub type StateMutationResult<T> = Result<T, StateMutationError>;
