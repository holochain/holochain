use crate::conductor::CellHandle;
use sx_core::error::SkunkError;
use std::{error::Error, fmt};

pub type ConductorResult<T> = Result<T, ConductorError>;

#[derive(Debug, PartialEq, Clone)]
pub enum ConductorError {
    InternalFailure(SkunkError),
    CellNotActive,
    CellAlreadyActive,
    CellNotInitialized,
    NoSuchCell(CellHandle),
    RequiredBridgeMissing(String),
}

impl Error for ConductorError {
    // not sure how to test this because dyn reference to the Error is not implementing PartialEq
    #[rustfmt::skip]
    fn cause(&self) -> Option<&dyn Error> {
        match self {
            ConductorError::InternalFailure(ref err) => Some(err),
            ConductorError::CellNotActive => None,
            ConductorError::CellAlreadyActive => None,
            ConductorError::CellNotInitialized => None,
            ConductorError::NoSuchCell(_) => None,
            ConductorError::RequiredBridgeMissing(_) => None,
        }
    }
}

impl fmt::Display for ConductorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = "Holochain Conductor Error";
        match self {
            ConductorError::InternalFailure(ref err) => write!(f, "{}: {}", prefix, err),
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
        }
    }
}

impl From<SkunkError> for ConductorError {
    fn from(error: SkunkError) -> Self {
        ConductorError::InternalFailure(error)
    }
}

impl From<ConductorError> for SkunkError {
    fn from(error: ConductorError) -> Self {
        SkunkError::new(error.to_string())
    }
}
