use super::{entry_def_store::error::EntryDefStoreError, interface::error::InterfaceError};
use crate::{conductor::cell::error::CellError, core::workflow::error::WorkflowError};
use holochain_state::error::DatabaseError;
use holochain_types::{app::AppId, cell::CellId};
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

    #[error("No conductor config found at this path: {0}")]
    ConfigMissing(PathBuf),

    #[error("Configuration consistency error: {0}")]
    ConfigError(String),

    #[error("Config deserialization error: {0}")]
    SerializationError(#[from] serde_yaml::Error),

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

    #[error("Workflow error: {0:?}")]
    WorkflowError(#[from] WorkflowError),

    // Box is to avoid cycle in error definition
    #[error(transparent)]
    InterfaceError(#[from] Box<InterfaceError>),

    #[error(transparent)]
    CreateAppFailed(#[from] CreateAppError),

    #[error("Failed to run genesis on the following cells in the app: {errors:?}")]
    GenesisFailed { errors: Vec<CellError> },

    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),

    #[error("Wasm code was not found in the wasm store")]
    WasmMissing,

    #[error("Tried to activate an app that was not installed")]
    AppNotInstalled,

    #[error("Tried to deactivate an app that was not active")]
    AppNotActive,

    #[error(transparent)]
    HolochainP2pError(#[from] holochain_p2p::HolochainP2pError),

    #[error(transparent)]
    EntryDefStoreError(#[from] EntryDefStoreError),

    #[error(transparent)]
    KeystoreError(#[from] holochain_keystore::KeystoreError),
}

#[derive(Error, Debug)]
pub enum CreateAppError {
    #[error("Failed to create the following cells in the {app_id} app: {errors:?}")]
    Failed {
        app_id: AppId,
        errors: Vec<CellError>,
    },
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
