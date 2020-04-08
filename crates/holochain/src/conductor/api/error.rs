//! Errors occurring during a [CellConductorApi] or [InterfaceApi] call

use sx_types::cell::CellId;
use thiserror::Error;

/// Errors occurring during a [CellConductorApi] or [InterfaceApi] call
#[derive(Error, Debug)]
pub enum ConductorApiError {
    /// Cell was referenced, but is missing from the conductor.
    #[error("Cell was referenced, but is missing from the conductor. CellId: {0:?}")]
    CellMissing(CellId),

    /// Miscellaneous error
    #[error("Miscellaneous error: {0}")]
    Todo(String),
}

/// Type alias
pub type ConductorApiResult<T> = Result<T, ConductorApiError>;
