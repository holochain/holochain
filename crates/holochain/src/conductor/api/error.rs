//! Errors occurring during a [CellConductorApi] or [InterfaceApi] call

use crate::conductor::error::ConductorError;
use crate::conductor::error::CreateAppError;
use crate::conductor::interface::error::InterfaceError;
use crate::conductor::CellError;
use crate::core::state::source_chain::SourceChainError;
use crate::core::state::workspace::WorkspaceError;
use crate::core::workflow::error::WorkflowError;
use crate::nucleus::ribosome::error::RibosomeError;
use holochain_serialized_bytes::prelude::*;
use holochain_state::error::DatabaseError;
use holochain_types::cell::CellId;
use thiserror::Error;

/// Errors occurring during a [CellConductorApi] or [InterfaceApi] call
#[derive(Error, Debug)]
pub enum ConductorApiError {
    /// Cell was referenced, but is missing from the conductor.
    #[error("Cell was referenced, but is missing from the conductor. CellId: {0:?}")]
    CellMissing(CellId),

    /// Cell was referenced, but is missing from the conductor.
    #[error(
        "A Cell attempted to use an CellConductorApi it was not given.\nAPI CellId: {api_cell_id:?}\nInvocation CellId: {invocation_cell_id:?}"
    )]
    ZomeCallInvocationCellMismatch {
        /// The CellId which is referenced by the CellConductorApi
        api_cell_id: CellId,
        /// The CellId which is referenced by the ZomeCallInvocation
        invocation_cell_id: CellId,
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
    DnaError(#[from] crate::nucleus::dna::DnaError),

    /// The Dna file path provided was invalid
    #[error("The Dna file path provided was invalid")]
    DnaReadError(String),

    /// KeystoreError
    #[error("KeystoreError: {0}")]
    KeystoreError(#[from] holochain_keystore::KeystoreError),

    /// Cell error
    #[error(transparent)]
    CellError(#[from] CellError),

    /// Error in the Interface
    #[error("An error occurred in the interface: {0:?}")]
    InterfaceError(#[from] InterfaceError),

    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
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

/// Error type that goes over the websocket wire.
/// This intends to be application developer facing
/// so it should be readable and relevant
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes, Clone)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum ExternalApiWireError {
    // TODO: B-01506 Constrain these errors so they are relevant to
    // application developers and what they would need
    // to react to using code (i.e. not just print)
    /// Any internal error
    InternalError(String),
    /// The input to the api failed to Deseralize
    Deserialization(String),
    /// The dna path provided was invalid
    DnaReadError(String),
    /// There was an error in the ribosome
    RibosomeError(String),
    /// Error activating app
    ActivateApp(String),
    /// The zome call is unauthorized
    ZomeCallUnauthorized(String),
}

impl ExternalApiWireError {
    /// Convert the error from the display.
    pub fn internal<T: std::fmt::Display>(e: T) -> Self {
        // Display format is used because
        // this version intended for users.
        ExternalApiWireError::InternalError(e.to_string())
    }
}

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
