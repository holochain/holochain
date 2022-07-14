use super::interface::error::InterfaceError;
use super::{entry_def_store::error::EntryDefStoreError, state::AppInterfaceId};
use crate::conductor::cell::error::CellError;
use crate::core::workflow::error::WorkflowError;
use holochain_conductor_api::conductor::ConductorConfigError;
use holochain_sqlite::error::DatabaseError;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmErrorInner;
use holochain_zome_types::cell::CellId;
use thiserror::Error;

pub type ConductorResult<T> = Result<T, ConductorError>;

#[derive(Error, Debug)]
pub enum ConductorError {
    #[error("Internal Cell error: {0}")]
    InternalCellError(#[from] CellError),

    #[error(transparent)]
    AppError(#[from] AppError),

    #[error(transparent)]
    AppBundleError(#[from] AppBundleError),

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

    #[error(transparent)]
    ConductorConfigError(#[from] ConductorConfigError),

    #[error("Configuration consistency error: {0}")]
    ConfigError(String),

    #[error("Config deserialization error: {0}")]
    SerializationError(#[from] serde_yaml::Error),

    #[error("Attempted to call into the conductor while it is shutting down")]
    ShuttingDown,

    #[error("Error while performing IO for the Conductor: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Error while trying to send a task to the task manager: {0}")]
    SubmitTaskError(String),

    #[error("ZomeError: {0}")]
    ZomeError(#[from] holochain_zome_types::zome::error::ZomeError),

    #[error("DnaError: {0}")]
    DnaError(#[from] holochain_types::dna::DnaError),

    #[error("Workflow error: {0:?}")]
    WorkflowError(#[from] WorkflowError),

    #[error("Attempted to add two app interfaces with the same id: {0:?}")]
    AppInterfaceIdCollision(AppInterfaceId),

    // Box is to avoid cycle in error definition
    #[error(transparent)]
    InterfaceError(#[from] Box<InterfaceError>),

    #[error("Failed to run genesis on the following cells in the app: {errors:?}")]
    GenesisFailed { errors: Vec<CellError> },

    #[error(transparent)]
    SerializedBytesError(#[from] holochain_serialized_bytes::SerializedBytesError),

    #[error("Wasm code was not found in the wasm store")]
    WasmMissing,

    #[error("Tried to access an app that was not installed: {0}")]
    AppNotInstalled(InstalledAppId),

    #[error("Tried to install an app using an already-used InstalledAppId: {0}")]
    AppAlreadyInstalled(InstalledAppId),

    #[error("Tried to perform an operation on an app that was not running: {0}")]
    AppNotRunning(InstalledAppId),

    #[error(transparent)]
    HolochainP2pError(#[from] holochain_p2p::HolochainP2pError),

    #[error(transparent)]
    EntryDefStoreError(#[from] EntryDefStoreError),

    #[error(transparent)]
    KeystoreError(#[from] holochain_keystore::KeystoreError),

    #[error(transparent)]
    KitsuneP2pError(#[from] kitsune_p2p::KitsuneP2pError),

    #[error(transparent)]
    MrBundleError(#[from] mr_bundle::error::MrBundleError),

    #[error(transparent)]
    StateQueryError(#[from] holochain_state::query::StateQueryError),

    #[error(transparent)]
    StateMutationError(#[from] holochain_state::mutations::StateMutationError),

    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),

    #[error(transparent)]
    RusqliteError(#[from] rusqlite::Error),

    #[error(transparent)]
    RibosomeError(#[from] crate::core::ribosome::error::RibosomeError),

    /// Other
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl ConductorError {
    /// promote a custom error type to a ConductorError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }
}

impl From<one_err::OneErr> for ConductorError {
    fn from(e: one_err::OneErr) -> Self {
        Self::other(e)
    }
}

impl From<ConductorError> for WasmErrorInner {
    fn from(e: ConductorError) -> Self {
        Self::Host(e.to_string())
    }
}
