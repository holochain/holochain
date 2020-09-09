use holochain_types::cell::CellId;
use holochain_zome_types::header::AppEntryType;
use thiserror::Error;

use crate::core::{present::PresentError, ribosome::error::RibosomeError};

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
