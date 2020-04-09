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

    /// Conductor errored during API call.
    #[error("Conductor returned an error while using a ConductorApi: {0:?}")]
    ConductorError(#[from] ConductorError),

    /// Miscellaneous error
    #[error("Miscellaneous error: {0}")]
    Todo(String),
}

/// Type alias
pub type ConductorApiResult<T> = Result<T, ConductorApiError>;
