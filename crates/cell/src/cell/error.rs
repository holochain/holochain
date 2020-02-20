use sx_types::error::SkunkError;
use crate::agent::error::SourceChainError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CellError {
    #[error("could not read from source chain")]
    SourceChainError(#[from] SourceChainError),

    #[error("generic error")]
    Generic(#[from] SkunkError)
}

pub type CellResult<T> = Result<T, CellError>;
