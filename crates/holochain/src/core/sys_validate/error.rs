use derive_more::Display;
use std::convert::TryFrom;

use super::SourceChainError;
use super::MAX_ENTRY_SIZE;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::entry_def_store::error::EntryDefStoreError;
use crate::core::validation::OutcomeOrError;
use crate::core::workflow::WorkflowError;
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
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ValidationOutcome {
    #[error("The record with signature {0:?} and action {1:?} was found to be counterfeit")]
    Counterfeit(Signature, Action),
    #[error("The action {1:?} is not found in the countersigning session data {0:?}")]
    ActionNotInCounterSigningSession(CounterSigningSessionData, NewEntryAction),
    #[error(transparent)]
    CounterSigningError(#[from] CounterSigningError),
    #[error("The dependency {0:?} was not found on the DHT")]
    DepMissingFromDht(AnyDhtHash),
    #[error("The entry def index for {0:?} was out of range")]
    EntryDefId(AppEntryDef),
    #[error("The entry has a different hash to the action's entry hash")]
    EntryHash,
    #[error(
        "The entry size {0} was larger than the MAX_ENTRY_SIZE {}",
        MAX_ENTRY_SIZE
    )]
    EntryTooLarge(usize),
    #[error("The entry has a different type to the action's entry type")]
    EntryTypeMismatch,
    #[error("The visibility for {0:?} didn't match the zome")]
    EntryVisibility(AppEntryDef),
    #[error(
        "The link tag size {0} was larger than the MAX_TAG_SIZE {}",
        super::MAX_TAG_SIZE
    )]
    TagTooLarge(usize),
    #[error("An op with non-private entry type is missing its entry data. Action: {0:?}, Op type: {1:?} Reason: {2}")]
    MalformedDhtOp(Box<Action>, ChainOpType, String),
    #[error("The action with {0:?} was expected to be a link add action")]
    NotCreateLink(ActionHash),
    #[error("The action was expected to be a new entry action but was {0:?}")]
    NotNewEntry(Action),
    #[error("The PreflightResponse signature was not valid {0:?}")]
    PreflightResponseSignature(PreflightResponse),
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
    #[error("Signature {0:?} failed to verify for Action {1:?}")]
    VerifySignature(Signature, Action),
    #[error("The zome index for {0:?} was out of range")]
    ZomeIndex(AppEntryDef),
}

impl ValidationOutcome {
    pub fn not_found<I: Into<AnyDhtHash> + Clone>(h: &I) -> Self {
        Self::DepMissingFromDht(h.clone().into())
    }

    /// Convert into a OutcomeOrError<ValidationOutcome, SysValidationError>
    /// and exit early
    pub fn into_outcome<T>(self) -> SysValidationOutcome<T> {
        Err(OutcomeOrError::Outcome(self))
    }
}

/// Context information for an invalid action to make it easier to trace in errors.
#[derive(Error, Debug, Display, PartialEq, Eq)]
#[display(
    fmt = "{} - with context seq={}, action_hash={:?}, action=[{}]",
    source,
    seq,
    action_hash,
    action_display
)]
pub struct PrevActionError {
    #[source]
    pub source: PrevActionErrorKind,
    pub seq: u32,
    pub action_hash: ActionHash,
    pub action_display: String,
}

impl<A: ChainItem> From<(PrevActionErrorKind, &A)> for PrevActionError {
    fn from((inner, action): (PrevActionErrorKind, &A)) -> Self {
        PrevActionError {
            source: inner,
            seq: action.seq(),
            action_hash: action.get_hash().clone().into(),
            action_display: action.to_display(),
        }
    }
}

impl From<(PrevActionErrorKind, Action)> for PrevActionError {
    fn from((inner, action): (PrevActionErrorKind, Action)) -> Self {
        PrevActionError {
            source: inner,
            seq: action.action_seq(),
            action_hash: action.to_hash(),
            action_display: format!("{}", action),
        }
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum PrevActionErrorKind {
    #[error("The previous action hash specified in an action doesn't match the actual previous action. Seq: {0}")]
    HashMismatch(u32),
    #[error("Root of source chain must be Dna")]
    InvalidRoot,
    #[error("Root of source chain must have a timestamp greater than the Dna's origin_time")]
    InvalidRootOriginTime,
    #[error("No more actions are allowed after a chain close")]
    ActionAfterChainClose,
    #[error("Previous action sequence number {1} != ({0} - 1)")]
    InvalidSeq(u32, u32),
    #[error("Action is not the first, so needs previous action")]
    MissingPrev,
    #[error("The previous action's timestamp is not before the current action's timestamp: {0:?} >= {1:?}")]
    Timestamp(Timestamp, Timestamp),
    #[error("The previous action's author does not match the current action's author: {0} != {1}")]
    Author(AgentPubKey, AgentPubKey),
    #[error("It is invalid for these two actions to be paired with each other. context: {0}, actions: {1:?}")]
    InvalidSuccessor(String, Box<(Action, Action)>),
}
