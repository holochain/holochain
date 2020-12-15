use holochain_lmdb::error::DatabaseError;
use thiserror::Error;

use crate::core::{state::cascade::error::CascadeError, SourceChainError};

#[derive(Error, Debug)]
pub enum PresentError {
    #[error(transparent)]
    CascadeError(#[from] CascadeError),
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),
    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
}

pub type PresentResult<T> = Result<T, PresentError>;
