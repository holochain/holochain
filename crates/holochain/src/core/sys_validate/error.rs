use super::SourceChainError;
use crate::core::state::cascade::error::CascadeError;
use holo_hash::AnyDhtHash;
use holochain_keystore::{KeystoreError, Signature};
use holochain_state::error::DatabaseError;
use holochain_zome_types::{header::EntryType, Header};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SysValidationError {
    #[error(transparent)]
    CascadeError(#[from] CascadeError),
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),
    #[error("The dependency {0:?} was not found on the DHT")]
    DepMissingFromDht(AnyDhtHash),
    #[error("The entry has a different hash to the header's entry hash")]
    EntryHash,
    #[error("The entry size {0} was bigger then the MAX_ENTRY_SIZE {1}")]
    EntryTooLarge(usize, usize),
    #[error("The entry has a different type to the header's entry type")]
    EntryType,
    #[error("The link tag size {0} was bigger then the MAX_TAG_SIZE {1}")]
    TagTooLarge(usize, usize),
    #[error("The header was expected to be a new entry header but was a {0:?}")]
    NotNewEntry(Header),
    #[error("The dependency {0:?} is not held")]
    NotHoldingDep(AnyDhtHash),
    #[error(transparent)]
    KeystoreError(#[from] KeystoreError),
    #[error(transparent)]
    PrevHeaderError(#[from] PrevHeaderError),
    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
    #[error("EntryUpdate original EntryType: {0:?} doesn't match new EntryType {1:?}")]
    UpdateTypeMismatch(EntryType, EntryType),
    #[error("Signature {0:?} failed to verify for Header {1:?}")]
    VerifySignature(Signature, Header),
}

pub type SysValidationResult<T> = Result<T, SysValidationError>;

#[derive(Error, Debug)]
pub enum PrevHeaderError {
    #[error("Root of source chain must be Dna")]
    InvalidRoot,
    #[error("Previous header sequence number {1} is not {0} - 1")]
    InvalidSeq(u32, u32),
    #[error("Previous header was missing from the metadata store")]
    MissingMeta,
    #[error("Header is not Dna so needs previous header")]
    MissingPrev,
    #[error("Previous header was missing from the element store")]
    MissingVault,
    #[error("The previous header's timestamp is not before the current header's timestamp")]
    Timestamp,
}
