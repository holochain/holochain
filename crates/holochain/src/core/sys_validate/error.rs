use std::convert::TryFrom;

use super::SourceChainError;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::entry_def_store::error::EntryDefStoreError;
use crate::core::validation::OutcomeOrError;
use crate::core::workflow::error::WorkflowError;
use crate::from_sub_error;
use holo_hash::ActionHash;
use holo_hash::AnyDhtHash;
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
/// <https://docs.rs/try-guard/0.2.0/try_guard/>
#[derive(Error, Debug)]
// TODO FIXME
#[allow(clippy::large_enum_variant)]
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
    #[error("Dna is missing for this hash {0:?}. Cannot validate without dna.")]
    DnaMissing(DnaHash),
    // NOTE: can remove this if SysValidationResult is replaced with SysValidationOutcome
    #[error(transparent)]
    ValidationOutcome(#[from] ValidationOutcome),
    #[error(transparent)]
    WorkflowError(#[from] Box<WorkflowError>),
    #[error(transparent)]
    WorkspaceError(#[from] WorkspaceError),
    #[error(transparent)]
    ConductorApiError(#[from] Box<ConductorApiError>),
    #[error("Expected Entry-based Action, but got: {0:?}")]
    NonEntryAction(Action),
}

impl From<CounterSigningError> for SysValidationError {
    fn from(counter_signing_error: CounterSigningError) -> Self {
        SysValidationError::ValidationOutcome(ValidationOutcome::CounterSigningError(
            counter_signing_error,
        ))
    }
}

// #[deprecated = "This will be replaced with SysValidationOutcome as we shouldn't treat outcomes as errors"]
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
    #[error("The record with signature {0:?} and action {1:?} was found to be counterfeit")]
    Counterfeit(Signature, Action),
    #[error("The action {1:?} is not found in the countersigning session data {0:?}")]
    ActionNotInCounterSigningSession(CounterSigningSessionData, NewEntryAction),
    #[error(transparent)]
    CounterSigningError(#[from] CounterSigningError),
    #[error("The dependency {0:?} was not found on the DHT")]
    DepMissingFromDht(AnyDhtHash),
    #[error("The app entry def {0:?} entry def id was out of range")]
    EntryDefId(AppEntryDef),
    #[error("The entry has a different hash to the action's entry hash")]
    EntryHash,
    #[error("The entry size {0} was larger than the MAX_ENTRY_SIZE {1}")]
    EntryTooLarge(usize, usize),
    #[error("The entry has a different type to the action's entry type")]
    EntryType,
    #[error("The app entry def {0:?} visibility didn't match the zome")]
    EntryVisibility(AppEntryDef),
    #[error("The link tag size {0} was larger than the MAX_TAG_SIZE {1}")]
    TagTooLarge(usize, usize),
    #[error("An op with non-private entry type is missing its entry data. Action: {0:?}, Op type: {1:?} Reason: {2}")]
    MalformedDhtOp(Box<Action>, DhtOpType, String),
    #[error("The action {0:?} was expected to be a link add action")]
    NotCreateLink(ActionHash),
    #[error("The action was expected to be a new entry action but was a {0:?}")]
    NotNewEntry(Action),
    #[error("The dependency {0:?} is not held")]
    NotHoldingDep(AnyDhtHash),
    #[error("The PreflightResponse signature was not valid {0:?}")]
    PreflightResponseSignature(PreflightResponse),
    #[error(transparent)]
    PrevActionError(#[from] PrevActionError),
    #[error("Private entry data should never be included in any op other than StoreEntry.")]
    PrivateEntryLeaked,
    #[error(
        "The DNA does not belong in this space! Action DNA hash: {0:?}, expected DNA hash: {1:?}"
    )]
    WrongDna(DnaHash, DnaHash),
    #[error("Update original EntryType: {0:?} doesn't match new EntryType {1:?}")]
    UpdateTypeMismatch(EntryType, EntryType),
    #[error("Signature {0:?} failed to verify for Action {1:?}")]
    VerifySignature(Signature, Action),
    #[error("The app entry def {0:?} zome index was out of range")]
    ZomeIndex(AppEntryDef),
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
pub enum PrevActionError {
    #[error("The previous action hash specified in an action doesn't match the actual previous action. Seq: {0}")]
    HashMismatch(u32),
    #[error("Root of source chain must be Dna")]
    InvalidRoot,
    #[error("Root of source chain must have a timestamp greater than the Dna's origin_time")]
    InvalidRootOriginTime,
    #[error("Previous action sequence number {1} != ({0} - 1)")]
    InvalidSeq(u32, u32),
    #[error("Previous action was missing from the metadata store")]
    MissingMeta(ActionHash),
    #[error("Action is not the first, so needs previous action")]
    MissingPrev,
    #[error("The previous action's timestamp is not before the current action's timestamp: {0:?} >= {1:?}")]
    Timestamp(Timestamp, Timestamp),
    #[error("It is invalid for these two actions to be paired with each other. context: {0}, actions: {1:?}")]
    InvalidSuccessor(String, Box<(Action, Action)>),
}
