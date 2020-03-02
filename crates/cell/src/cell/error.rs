use sx_types::error::SkunkError;
// use crate::agent::error::SourceChainError;
use thiserror::Error;
use sx_state::error::WorkspaceError;

#[derive(Error, Debug)]
pub enum CellError {
    #[error("error dealing with workspace state: {0}")]
    WorkspaceError(#[from] WorkspaceError),

    // TODO
    // #[error("could not read from source chain")]
    // SourceChainError(#[from] SourceChainError),

    #[error("generic error")]
    Generic(#[from] SkunkError)
}

pub type CellResult<T> = Result<T, CellError>;
