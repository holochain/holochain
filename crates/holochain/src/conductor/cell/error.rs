use sx_state::error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CellError {
    #[error("error dealing with workspace state: {0}")]
    DatabaseError(#[from] DatabaseError),
}

pub type CellResult<T> = Result<T, CellError>;
