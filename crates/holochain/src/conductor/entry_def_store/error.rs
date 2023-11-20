#![allow(missing_docs)]

use crate::core::ribosome::error::RibosomeError;
use holochain_zome_types::zome::ZomeName;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EntryDefStoreError {
    #[error(transparent)]
    DnaError(#[from] RibosomeError),
    #[error("Unable to retrieve DNA from the RibosomeStore. DnaHash: {0}")]
    DnaFileMissing(holo_hash::DnaHash),
    #[error(
        "Too many entry definitions in a single zome. Entry definitions are limited to 255 per zome"
    )]
    TooManyEntryDefs,
    #[error("The entry def callback for {0} failed because {1}")]
    CallbackFailed(ZomeName, String),
    #[error("Entry type is missing from the zome types map on the Ribosome")]
    EntryTypeMissing,
    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),
}

pub type EntryDefStoreResult<T> = Result<T, EntryDefStoreError>;
