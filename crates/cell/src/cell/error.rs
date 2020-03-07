use sx_types::error::SkunkError;
// use crate::state::source_chain::SourceChainError;
use sx_state::error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CellError {
    #[error("error dealing with workspace state: {0}")]
    DatabaseError(#[from] DatabaseError),

    // TODO
    // #[error("could not read from source chain")]
    // SourceChainError(#[from] SourceChainError),
    #[error("generic error")]
    Generic(#[from] SkunkError),
}

pub type CellResult<T> = Result<T, CellError>;
