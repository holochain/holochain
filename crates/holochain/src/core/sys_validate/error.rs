use super::SourceChainError;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::entry_def_store::error::EntryDefStoreError;
use crate::core::validation::OutcomeOrError;
use crate::core::workflow::WorkflowError;
use holo_hash::ActionHash;
use holo_hash::AnyDhtHash;
use holochain_keystore::KeystoreError;
use holochain_sqlite::error::DatabaseError;
use holochain_types::prelude::*;
use holochain_zome_types::countersigning::CounterSigningError;
use holochain_zome_types::countersigning::CounterSigningSessionData;
use std::convert::TryFrom;
use thiserror::Error;

/// Validation can result in either
/// - An Error
/// - Failed validation
/// - Successful validation
///
/// It is a lot cleaner to express this using
/// ? try's unfortunately try for custom types is
/// unstable but when it lands we should use:
/// <https://docs.rs/try-guard/0.2.0/try_guard/>
#[allow(missing_docs)]
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
    #[error("Dna is missing for this hash {0:?}. Cannot validate without dna.")]
    DnaMissing(DnaHash),
    // NOTE: can remove this if SysValidationResult is replaced with SysValidationOutcome
    #[error(transparent)]
    ValidationOutcome(#[from] ValidationOutcome),
    #[error(transparent)]
    WorkflowError(#[from] Box<WorkflowError>),
    #[error(transparent)]
    ConductorApiError(#[from] Box<ConductorApiError>),
    #[error("Expected Entry-based Action, but got: {0:?}")]
    NonEntryAction(Box<Action>),
}

impl From<CounterSigningError> for SysValidationError {
    fn from(counter_signing_error: CounterSigningError) -> Self {
        SysValidationError::ValidationOutcome(ValidationOutcome::CounterSigningError(
            counter_signing_error,
        ))
    }
}

/// A result type for sys validation with the error type [`SysValidationError`]
pub type SysValidationResult<T> = Result<T, SysValidationError>;

/// Return either:
/// - an Ok result
/// - ValidationOutcome
/// - SysValidationError
pub type SysValidationOutcome<T> = Result<T, OutcomeOrError<ValidationOutcome, SysValidationError>>;

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
#[allow(missing_docs)]
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ValidationOutcome {
    #[error("The record with signature {0:?} and action {1:?} was found to be counterfeit")]
    CounterfeitAction(Signature, Box<Action>),
    #[error("A warrant op was found to be counterfeit. Warrant: {0:?}")]
    CounterfeitWarrant(Box<Warrant>),
    #[error("A warrant op was found to be invalid. Reason: {1}, Warrant: {0:?}")]
    InvalidWarrant(Box<Warrant>, String),
    #[error("The action {1:?} is not found in the countersigning session data {0:?}")]
    ActionNotInCounterSigningSession(Box<CounterSigningSessionData>, Box<NewEntryAction>),
    #[error(transparent)]
    CounterSigningError(#[from] CounterSigningError),
    #[error("The dependency {0:?} was not found on the DHT")]
    DepMissingFromDht(AnyDhtHash),
    #[error("Agent key {0} invalid")]
    InvalidAgentKey(AgentPubKey),
    #[error("The entry def index for {0:?} was out of range")]
    EntryDefId(AppEntryDef),
    #[error("The entry has a different hash to the action's entry hash")]
    EntryHash,
    #[error(
        "The entry size {0} was larger than the MAX_ENTRY_SIZE {max}",
        max = super::MAX_ENTRY_SIZE
    )]
    EntryTooLarge(usize),
    #[error("The entry has a different type to the action's entry type")]
    EntryTypeMismatch,
    #[error("The visibility for {0:?} didn't match the zome")]
    EntryVisibility(AppEntryDef),
    #[error(
        "The link tag size {0} was larger than the MAX_TAG_SIZE {max}",
        max = super::MAX_TAG_SIZE
    )]
    TagTooLarge(usize),
    #[error("An op with non-private entry type is missing its entry data. Action: {0:?}, Op type: {1:?} Reason: {2}")]
    MalformedDhtOp(Box<Action>, ChainOpType, String),
    #[error("The action with {0:?} was expected to be a link add action")]
    NotCreateLink(ActionHash),
    #[error("The action was expected to be a new entry action but was {0:?}")]
    NotNewEntry(Box<Action>),
    #[error("The PreflightResponse signature was not valid {0:?}")]
    PreflightResponseSignature(Box<PreflightResponse>),
    #[error(transparent)]
    PrevActionError(#[from] PrevActionError),
    #[error("Private entry data should never be included in any op other than StoreEntry.")]
    PrivateEntryLeaked,
    #[error("The DNA does not belong in this space! Action has {0:?}, expected {1:?}")]
    WrongDna(DnaHash, DnaHash),
    #[error("Update original: {0:?} doesn't match new: {1:?}")]
    UpdateTypeMismatch(EntryType, EntryType),
    #[error("Update original {0:?} doesn't match the {1:?} in the update")]
    UpdateHashMismatch(EntryHash, EntryHash),
    #[error("The zome index for {0:?} was out of range")]
    ZomeIndex(AppEntryDef),
}

impl ValidationOutcome {
    /// Convert into a OutcomeOrError<ValidationOutcome, SysValidationError>
    /// and exit early
    pub fn into_outcome<T>(self) -> SysValidationOutcome<T> {
        Err(OutcomeOrError::Outcome(self))
    }

    /// The outcome is pending further information, so no determination can be made at this time.
    /// If this is false, then the outcome is determinate, meaning we can reject validation now.
    pub fn is_indeterminate(&self) -> bool {
        if let ValidationOutcome::CounterfeitAction(_, _)
        | ValidationOutcome::CounterfeitWarrant(_) = self
        {
            // Just a helpful assertion for us
            unreachable!("Counterfeit ops are dropped before sys validation")
        }
        matches!(self, Self::DepMissingFromDht(_))
    }
}
