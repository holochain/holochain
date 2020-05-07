use super::{dna_store::error::DnaStoreError, interface::error::InterfaceError};
use crate::conductor::cell::error::CellError;
use holochain_state::error::DatabaseError;
use holochain_types::cell::{CellHandle, CellId};
use std::path::PathBuf;
use thiserror::Error;

pub type ConductorResult<T> = Result<T, ConductorError>;

#[derive(Error, Debug)]
pub enum ConductorError {
    #[error("Internal Cell error: {0}")]
    InternalCellError(#[from] CellError),

    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error("Cell is not active yet.")]
    CellNotActive,

    #[error("Cell is already active.")]
    CellAlreadyActive,

    #[error("Cell is not initialized.")]
    CellNotInitialized,

    #[error("Cell was referenced, but is missing from the conductor. CellId: {0:?}")]
    CellMissing(CellId),

    #[error("No such cell: {0}")]
    NoSuchCell(CellHandle),

    #[error("No conductor config found at this path: {0}")]
    ConfigMissing(PathBuf),

    #[error("Configuration consistency error: {0}")]
    ConfigError(String),

    #[error("Config serialization error: {0}")]
    DeserializationError(#[from] toml::de::Error),

    #[error("Config deserialization error: {0}")]
    SerializationError(#[from] toml::ser::Error),

    #[error("Attempted to call into the conductor while it is shutting down")]
    ShuttingDown,

    #[error("Miscellaneous error: {0}")]
    Todo(String),

    #[error("Error while performing IO for the Conductor: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Error while trying to send a task to the task manager: {0}")]
    SubmitTaskError(String),

    #[error("DnaError: {0}")]
    DnaError(#[from] holochain_types::dna::DnaError),

    #[error("DNA store error: {0:?}")]
    DnaStoreError(#[from] DnaStoreError),

    // Box is to avoid cycle in error definition
    #[error(transparent)]
    InterfaceError(#[from] Box<InterfaceError>),
}

// TODO: can this be removed?
impl From<String> for ConductorError {
    fn from(s: String) -> Self {
        ConductorError::Todo(s)
    }
}

impl PartialEq for ConductorError {
    fn eq(&self, other: &Self) -> bool {
        use ConductorError::*;
        match (self, other) {
            (InternalCellError(a), InternalCellError(b)) => a.to_string() == b.to_string(),
            (InternalCellError(_), _) => false,
            (_, InternalCellError(_)) => false,
            (a, b) => a == b,
        }
    }
}
