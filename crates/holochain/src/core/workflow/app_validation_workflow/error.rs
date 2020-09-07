use thiserror::Error;

use crate::core::{ribosome::error::RibosomeError, ValidationError};

#[derive(Error, Debug)]
pub enum AppValidationError {
    #[error(transparent)]
    RibosomeError(#[from] RibosomeError),
    #[error(transparent)]
    ValidationError(#[from] ValidationError),
}

pub type AppValidationResult<T> = Result<T, AppValidationError>;
