//! Errors occurring during a [CellConductorApi] or [InterfaceApi] call

use crate::conductor::error::ConductorError;
use crate::conductor::error::CreateAppError;
use crate::conductor::interface::error::InterfaceError;
use crate::conductor::CellError;
use crate::core::ribosome::error::RibosomeError;
use crate::core::workflow::error::WorkflowError;
use holo_hash::DnaHash;
use holochain_sqlite::error::DatabaseError;
use holochain_state::source_chain::SourceChainError;
use holochain_state::workspace::WorkspaceError;
use holochain_types::prelude::*;
use holochain_zome_types::cell::CellId;
use mr_bundle::error::MrBundleError;
use thiserror::Error;

/// Errors occurring during a [CellConductorApi] or [InterfaceApi] call
#[derive(Error, Debug)]
pub enum ConductorApiError {
    /// The Dna for this Cell is not installed in the conductor.
    #[error("The Dna for this Cell is not installed in the conductor! DnaHash: {0}")]
    DnaMissing(DnaHash),

    /// Cell was referenced, but is missing from the conductor.
    #[error(
        "A Cell attempted to use an CellConductorApi it was not given.\nAPI CellId: {api_cell_id:?}\nInvocation CellId: {call_cell_id:?}"
    )]
    ZomeCallCellMismatch {
        /// The CellId which is referenced by the CellConductorApi
        api_cell_id: CellId,
        /// The CellId which is referenced by the ZomeCallInvocation
        call_cell_id: CellId,
    },

    /// Conductor threw an error during API call.
    #[error("Conductor returned an error while using a ConductorApi: {0:?}")]
    ConductorError(#[from] ConductorError),

    /// Io error.
    #[error("Io error while using a Interface Api: {0:?}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error while using a InterfaceApi: {0:?}")]
    SerializationError(#[from] SerializationError),

    /// Database error
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    /// Workspace error.
    // TODO: Can be avoided if we can move workspace creation into the workflow
    #[error(transparent)]
    WorkspaceError(#[from] WorkspaceError),

    /// Workflow error.
    // TODO: perhaps this Box can be avoided with further reorganization
    #[error(transparent)]
    WorkflowError(#[from] Box<WorkflowError>),

    /// DnaError
    #[error("DnaError: {0}")]
    DnaError(#[from] holochain_types::dna::DnaError),

    /// The Dna file path provided was invalid
    #[error("The Dna file path provided was invalid")]
    DnaReadError(String),

    /// KeystoreError
    #[error("KeystoreError: {0}")]
    KeystoreError(#[from] holochain_keystore::KeystoreError),

    /// Cell error
    #[error(transparent)]
    CellError(#[from] CellError),

    /// App error
    #[error(transparent)]
    AppError(#[from] AppError),

    /// Error in the Interface
    #[error("An error occurred in the interface: {0:?}")]
    InterfaceError(#[from] InterfaceError),

    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),

    #[error(transparent)]
    AppBundleError(#[from] AppBundleError),

    #[error(transparent)]
    MrBundleError(#[from] MrBundleError),

    #[error(transparent)]
    JsonDumpError(#[from] serde_json::Error),

    #[error(transparent)]
    StateQueryError(#[from] holochain_state::query::StateQueryError),

    #[error(transparent)]
    StateMutationError(#[from] holochain_state::mutations::StateMutationError),

    #[error(transparent)]
    RusqliteError(#[from] rusqlite::Error),

    /// Other
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl ConductorApiError {
    /// promote a custom error type to a KitsuneP2pError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }
}

/// All the serialization errors that can occur
#[derive(Error, Debug)]
pub enum SerializationError {
    /// Denotes inability to move into or out of SerializedBytes
    #[error(transparent)]
    Bytes(#[from] holochain_serialized_bytes::SerializedBytesError),

    /// Denotes inability to parse a UUID
    #[error(transparent)]
    Uuid(#[from] uuid::parser::ParseError),
}

/// Type alias
pub type ConductorApiResult<T> = Result<T, ConductorApiError>;

pub use holochain_conductor_api::ExternalApiWireError;

impl From<ConductorApiError> for ExternalApiWireError {
    fn from(err: ConductorApiError) -> Self {
        match err {
            ConductorApiError::DnaReadError(e) => ExternalApiWireError::DnaReadError(e),
            e => ExternalApiWireError::internal(e),
        }
    }
}

impl From<SerializationError> for ExternalApiWireError {
    fn from(e: SerializationError) -> Self {
        ExternalApiWireError::Deserialization(format!("{:?}", e))
    }
}

impl From<RibosomeError> for ExternalApiWireError {
    fn from(e: RibosomeError) -> Self {
        ExternalApiWireError::RibosomeError(e.to_string())
    }
}

impl From<CreateAppError> for ExternalApiWireError {
    fn from(e: CreateAppError) -> Self {
        ExternalApiWireError::ActivateApp(e.to_string())
    }
}
