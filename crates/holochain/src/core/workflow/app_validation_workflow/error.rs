use holochain_p2p::HolochainP2pError;
use holochain_types::prelude::*;
use thiserror::Error;

use crate::conductor::entry_def_store::error::EntryDefStoreError;
use crate::core::ribosome::error::RibosomeError;
use crate::core::validation::OutcomeOrError;
use crate::core::SourceChainError;
use crate::from_sub_error;

use super::types::Outcome;

#[derive(Error, Debug)]
pub enum AppValidationError {
    #[error(transparent)]
    CascadeError(#[from] holochain_cascade::error::CascadeError),
    #[error("Dna is missing {0:?}. Cannot validate without dna.")]
    DnaMissing(DnaHash),
    #[error(transparent)]
    DhtOpError(#[from] DhtOpError),
    #[error(transparent)]
    EntryDefStoreError(#[from] EntryDefStoreError),
    #[error(transparent)]
    HolochainP2pError(#[from] HolochainP2pError),
    #[error("Links cannot be called on multiple zomes for validation")]
    LinkMultipleZomes,
    #[error(transparent)]
    RibosomeError(#[from] RibosomeError),
    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
    #[error("The app entry type {0:?} zome index was out of range")]
    ZomeIndex(ZomeIndex),
}

pub type AppValidationResult<T> = Result<T, AppValidationError>;
/// This is a way to return a success or immediately exit with an outcome
/// or immediately exit with an error
pub(super) type AppValidationOutcome<T> = Result<T, OutcomeOrError<Outcome, AppValidationError>>;

impl<T> From<AppValidationError> for OutcomeOrError<T, AppValidationError> {
    fn from(e: AppValidationError) -> Self {
        OutcomeOrError::Err(e)
    }
}
use holochain_cascade::error::CascadeError;
// These need to match the #[from] in AppValidationError
from_sub_error!(AppValidationError, RibosomeError);
from_sub_error!(AppValidationError, CascadeError);
from_sub_error!(AppValidationError, EntryDefStoreError);
from_sub_error!(AppValidationError, SourceChainError);
from_sub_error!(AppValidationError, DhtOpError);
