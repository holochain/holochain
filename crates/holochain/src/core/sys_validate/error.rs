use super::SourceChainError;
use crate::{
    conductor::entry_def_store::error::EntryDefStoreError,
    core::state::cascade::error::CascadeError, core::workflow::error::WorkflowError,
};
use holo_hash::{AnyDhtHash, HeaderHash};
use holochain_keystore::KeystoreError;
use holochain_state::error::DatabaseError;
use holochain_types::cell::CellId;
use holochain_zome_types::signature::Signature;
use holochain_zome_types::{
    header::{AppEntryType, EntryType},
    Header,
};
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
    CascadeError(#[from] CascadeError),
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
    #[error(transparent)]
    ValidationOutcome(#[from] ValidationOutcome),
    #[error(transparent)]
    WorkflowError(#[from] Box<WorkflowError>),
}

pub type SysValidationResult<T> = Result<T, SysValidationError>;

// TODO: use try guard crate to refactor this so it's not an "Error"
// https://docs.rs/try-guard/0.2.0/try_guard/
/// All the outcomes that can come from validation
/// This is not an error type it is the outcome of
/// failed validation.
#[derive(Error, Debug)]
pub enum ValidationOutcome {
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

#[derive(Error, Debug)]
pub enum PrevHeaderError {
    #[error("Root of source chain must be Dna")]
    InvalidRoot,
    #[error("Previous header sequence number {1} is not {0} - 1")]
    InvalidSeq(u32, u32),
    #[error("Previous header was missing from the metadata store")]
    MissingMeta(HeaderHash),
    #[error("Header is not Dna so needs previous header")]
    MissingPrev,
    #[error("The previous header's timestamp is not before the current header's timestamp")]
    Timestamp,
}
