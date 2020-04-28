//! Errors occurring during a [CellConductorApi] or [InterfaceApi] call

use crate::{conductor::error::ConductorError, core::workflow::runner::error::WorkflowRunError};
use holochain_serialized_bytes::prelude::*;
use holochain_types::cell::CellId;
use thiserror::Error;

/// Errors occurring during a [CellConductorApi] or [InterfaceApi] call
#[derive(Error, Debug)]
pub enum ConductorApiError {
    /// Cell was referenced, but is missing from the conductor.
    #[error("Cell was referenced, but is missing from the conductor. CellId: {0:?}")]
    CellMissing(CellId),

    /// Cell was referenced, but is missing from the conductor.
    #[error("A Cell attempted to use an CellConductorApi it was not given.\nAPI CellId: {api_cell_id:?}\nInvocation CellId: {invocation_cell_id:?}")]
    ZomeInvocationCellMismatch {
        /// The CellId which is referenced by the CellConductorApi
        api_cell_id: CellId,
        /// The CellId which is referenced by the ZomeInvocation
        invocation_cell_id: CellId,
    },

    /// Conductor threw an error during API call.
    #[error("Conductor returned an error while using a ConductorApi: {0:?}")]
    ConductorError(#[from] ConductorError),

    /// Miscellaneous error
    #[error("Miscellaneous error: {0}")]
    Todo(String),

    /// Io error.
    #[error("Io error while using a Interface Api: {0:?}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error while using a InterfaceApi: {0:?}")]
    SerializationError(#[from] SerializationError),

    /// The Dna file path provided was invalid
    #[error("The Dna file path provided was invalid")]
    DnaReadError(String),

    /// Error in the workflow
    #[error("An error occurred while running the workflow: {0:?}")]
    WorkflowRunError(#[from] WorkflowRunError),
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
#[serde(rename = "snake-case", tag = "type", content = "data")]
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
