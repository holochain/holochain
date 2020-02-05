use crate::conductor::CellHandle;
use std::fmt;
use sx_types::error::SkunkError;
use thiserror::Error;

pub type ConductorResult<T> = Result<T, ConductorError>;

#[derive(Error, PartialEq, Debug)]
pub enum ConductorError {
    InternalCellError(#[from] SkunkError),
    CellNotActive,
    CellAlreadyActive,
    CellNotInitialized,
    NoSuchCell(CellHandle),
    RequiredBridgeMissing(String),
    ConfigError(String),
    Misc(String),
}

impl From<String> for ConductorError {
    fn from(s: String) -> Self {
        ConductorError::Misc(s)
    }
}

impl fmt::Display for ConductorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = "Holochain Conductor Error";
        match self {
            ConductorError::InternalCellError(e) => {
                write!(f, "{}: Internal Cell error: {:?}", prefix, e)
            }
            ConductorError::CellNotActive => write!(f, "{}: Cell is not active yet.", prefix),
            ConductorError::CellAlreadyActive => write!(f, "{}: Cell is already active.", prefix),
            ConductorError::CellNotInitialized => write!(f, "{}: Cell is not initialized.", prefix),
            ConductorError::NoSuchCell(handle) => write!(
                f,
                "{}: Cell with handle '{}' does not exist",
                prefix, handle
            ),
            ConductorError::RequiredBridgeMissing(handle) => write!(
                f,
                "{}: Required bridge is not present/started: {}",
                prefix, handle
            ),
            ConductorError::ConfigError(reason) => {
                write!(f, "{}: Configuration error: {}", prefix, reason)
            }
            ConductorError::Misc(reason) => {
                write!(f, "{}: Miscellaneous error: {}", prefix, reason)
            }
        }
    }
}
