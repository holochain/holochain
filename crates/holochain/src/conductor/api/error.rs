#![deny(missing_docs)]
//! Errors occurring during a [CellConductorApi] or [InterfaceApi] call

use crate::conductor::error::ConductorError;
use sx_types::cell::CellId;
use thiserror::Error;

/// Errors occurring during a [CellConductorApi] or [InterfaceApi] call
#[derive(Error, Debug)]
pub enum ConductorApiError {
    /// Cell was referenced, but is missing from the conductor.
    #[error("Cell was referenced, but is missing from the conductor. CellId: {0:?}")]
    CellMissing(CellId),

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
}

/// All the serialization errors that can occur
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum SerializationError {
    #[error(transparent)]
    Bytes(#[from] holochain_serialized_bytes::SerializedBytesError),
    #[error(transparent)]
    Uuid(#[from] uuid::parser::ParseError),
}

/// Type alias
pub type ConductorApiResult<T> = Result<T, ConductorApiError>;
