use sx_types::agent::CellId;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConductorApiError {
    #[error("Cell was referenced, but is missing from the conductor. CellId: {0:?}")]
    CellMissing(CellId),

    #[error("Miscellaneous error: {0}")]
    Misc(String),
}

pub type ConductorApiResult<T> = Result<T, ConductorApiError>;
