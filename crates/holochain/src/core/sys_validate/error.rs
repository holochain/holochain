use std::convert::TryFrom;

use super::SourceChainError;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::entry_def_store::error::EntryDefStoreError;
use crate::core::validation::OutcomeOrError;
use crate::core::workflow::error::WorkflowError;
use crate::from_sub_error;
use holo_hash::AnyDhtHash;
use holo_hash::HeaderHash;
use holochain_keystore::KeystoreError;
use holochain_sqlite::error::DatabaseError;
use holochain_state::workspace::WorkspaceError;
use holochain_types::prelude::*;
use holochain_zome_types::countersigning::CounterSigningError;
use holochain_zome_types::countersigning::CounterSigningSessionData;
use thiserror::Error;

/// Validation can result in either
/// - An Error
/// - Failed validation
/// - Successful validation
/// It is a lot cleaner to express this using
/// ? try's unfortunately try for custom types is
/// unstable but when it lands we should use:
/// https://docs.rs/try-guard/0.2.0/try_guard/
#[derive(Error, Debug)]
pub enum SysValidationError {
    #[error(transparent)]
    CascadeError(#[from] holochain_cascade::error::CascadeError),
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),
    #[error(transparent)]
    EntryDefStoreError(#[from] EntryDefStoreError),
    #[error(transparent)]
    KeystoreError(#[from] KeystoreError),
    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
    #[error("Dna is missing for this cell {0:?}. Cannot validate without dna.")]
    DnaMissing(CellId),
    // TODO: Remove this when SysValidationResult is replace with SysValidationOutcome
    #[error(transparent)]
    ValidationOutcome(#[from] ValidationOutcome),
    #[error(transparent)]
    WorkflowError(#[from] Box<WorkflowError>),
    #[error(transparent)]
    WorkspaceError(#[from] WorkspaceError),
    #[error(transparent)]
    ConductorApiError(#[from] Box<ConductorApiError>),
}

impl From<CounterSigningError> for SysValidationError {
    fn from(counter_signing_error: CounterSigningError) -> Self {
        SysValidationError::ValidationOutcome(ValidationOutcome::CounterSigningError(
            counter_signing_error,
        ))
    }
}

#[deprecated = "This will be replaced with SysValidationOutcome as we shouldn't treat outcomes as errors"]
pub type SysValidationResult<T> = Result<T, SysValidationError>;

/// Return either:
/// - an Ok result
/// - ValidationOutcome
/// - SysValidationError
pub type SysValidationOutcome<T> = Result<T, OutcomeOrError<ValidationOutcome, SysValidationError>>;

from_sub_error!(SysValidationError, WorkspaceError);

impl<T> From<SysValidationError> for OutcomeOrError<T, SysValidationError> {
    fn from(e: SysValidationError) -> Self {
        OutcomeOrError::Err(e)
    }
}

/// Turn the OutcomeOrError into an Outcome or and Error
/// This is the best way to convert into an outcome or
/// exit early with a real error
impl<E> TryFrom<OutcomeOrError<ValidationOutcome, E>> for ValidationOutcome {
    type Error = E;

    fn try_from(value: OutcomeOrError<ValidationOutcome, E>) -> Result<Self, Self::Error> {
        match value {
            OutcomeOrError::Outcome(o) => Ok(o),
            OutcomeOrError::Err(e) => Err(e),
        }
    }
}

// TODO: use try guard crate to refactor this so it's not an "Error"
// https://docs.rs/try-guard/0.2.0/try_guard/
/// All the outcomes that can come from validation
/// This is not an error type it is the outcome of
/// failed validation.
#[derive(Error, Debug)]
pub enum ValidationOutcome {
    #[error("The element with signature {0:?} and header {1:?} was found to be counterfeit")]
    Counterfeit(Signature, Header),
    #[error("The header {1:?} is not found in the countersigning session data {0:?}")]
    HeaderNotInCounterSigningSession(CounterSigningSessionData, NewEntryHeader),
    #[error(transparent)]
    CounterSigningError(#[from] CounterSigningError),
    #[error("The dependency {0:?} was not found on the DHT")]
    DepMissingFromDht(AnyDhtHash),
    #[error("The app entry type {0:?} entry def id was out of range")]
    EntryDefId(AppEntryType),
    #[error("The entry has a different hash to the header's entry hash")]
    EntryHash,
    #[error("The entry size {0} was bigger then the MAX_ENTRY_SIZE {1}")]
    EntryTooLarge(usize, usize),
    #[error("The entry has a different type to the header's entry type")]
    EntryType,
    #[error("The app entry type {0:?} visibility didn't match the zome")]
    EntryVisibility(AppEntryType),
    #[error("The link tag size {0} was bigger then the MAX_TAG_SIZE {1}")]
    TagTooLarge(usize, usize),
    #[error("The header {0:?} was expected to be a link add header")]
    NotCreateLink(HeaderHash),
    #[error("The header was expected to be a new entry header but was a {0:?}")]
    NotNewEntry(Header),
    #[error("The dependency {0:?} is not held")]
    NotHoldingDep(AnyDhtHash),
    #[error("The PreflightResponse signature was not valid {0:?}")]
    PreflightResponseSignature(PreflightResponse),
    #[error(transparent)]
    PrevHeaderError(#[from] PrevHeaderError),
    #[error("StoreEntry should not be gossiped for private entries")]
    PrivateEntry,
    #[error("Update original EntryType: {0:?} doesn't match new EntryType {1:?}")]
    UpdateTypeMismatch(EntryType, EntryType),
    #[error("Signature {0:?} failed to verify for Header {1:?}")]
    VerifySignature(Signature, Header),
    #[error("The app entry type {0:?} zome id was out of range")]
    ZomeId(AppEntryType),
}

impl ValidationOutcome {
    pub fn not_holding<I: Into<AnyDhtHash> + Clone>(h: &I) -> Self {
        Self::NotHoldingDep(h.clone().into())
    }
    pub fn not_found<I: Into<AnyDhtHash> + Clone>(h: &I) -> Self {
        Self::DepMissingFromDht(h.clone().into())
    }

    /// Convert into a OutcomeOrError<ValidationOutcome, SysValidationError>
    /// and exit early
    pub fn into_outcome<T>(self) -> SysValidationOutcome<T> {
        Err(OutcomeOrError::Outcome(self))
    }
}

#[derive(Error, Debug)]
pub enum PrevHeaderError {
    #[error("Root of source chain must be Dna")]
    InvalidRoot,
    #[error("Previous header sequence number {1} != ({0} - 1)")]
    InvalidSeq(u32, u32),
    #[error("Previous header was missing from the metadata store")]
    MissingMeta(HeaderHash),
    #[error("Header is not Dna so needs previous header")]
    MissingPrev,
    #[error("The previous header's timestamp is not before the current header's timestamp")]
    Timestamp,
}
