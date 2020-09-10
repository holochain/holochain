use holochain_types::cell::CellId;
use holochain_zome_types::header::AppEntryType;
use thiserror::Error;

use crate::{
    core::validation::OutcomeOrError,
    core::{present::PresentError, ribosome::error::RibosomeError},
    from_sub_error,
};

use super::types::Outcome;

#[derive(Error, Debug)]
pub enum AppValidationError {
    #[error("Dna is missing for this cell {0:?}. Cannot validate without dna.")]
    DnaMissing(CellId),
    #[error(transparent)]
    PresentError(#[from] PresentError),
    #[error(transparent)]
    RibosomeError(#[from] RibosomeError),
    #[error("The app entry type {0:?} zome id was out of range")]
    ZomeId(AppEntryType),
}

pub type AppValidationResult<T> = Result<T, AppValidationError>;
pub(super) type AppValidationOutcome<T> = Result<T, OutcomeOrError<Outcome, AppValidationError>>;

impl<T> From<AppValidationError> for OutcomeOrError<T, AppValidationError> {
    fn from(e: AppValidationError) -> Self {
        OutcomeOrError::Err(e)
    }
}

from_sub_error!(AppValidationError, PresentError);
from_sub_error!(AppValidationError, RibosomeError);
