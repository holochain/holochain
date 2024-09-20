use holochain_sqlite::error::DatabaseError;
use thiserror::Error;

use crate::{mutations::StateMutationError, query::StateQueryError};

#[derive(Error, Debug)]
pub enum EventError {
    #[error(transparent)]
    SqlError(#[from] holochain_sqlite::rusqlite::Error),

    #[error(transparent)]
    StateQueryError(#[from] StateQueryError),

    #[error(transparent)]
    StateMutationError(#[from] StateMutationError),

    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error("The database is not in a state which corresponds to an Event stream: {0}")]
    BadDatabaseState(String),
}

pub type EventResult<T> = Result<T, EventError>;
