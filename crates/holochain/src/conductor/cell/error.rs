use holochain_state::error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CellError {
    #[error("error dealing with workspace state: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("The Dna was not found in the store")]
    DnaMissing,
}

pub type CellResult<T> = Result<T, CellError>;
