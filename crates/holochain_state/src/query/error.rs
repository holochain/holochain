use thiserror::Error;
#[derive(Error, Debug)]
pub enum StateQueryError {
    #[error(transparent)]
    Sql(#[from] holochain_sqlite::rusqlite::Error),
    #[error(transparent)]
    Infallible(#[from] std::convert::Infallible),
    #[error(transparent)]
    DatabaseError(#[from] holochain_sqlite::error::DatabaseError),
}

pub type StateQueryResult<T> = Result<T, StateQueryError>;
