use crate::{cell::CellT, conductor::ConductorT, interface::CellConductorInterfaceT};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConductorApiError
// where
//     CellError: std::error::Error + 'static,
{
    // #[error("CellError: {0}")]
    // CellError(#[from] CellError),

    #[error("Miscellaneous error: {0}")]
    Misc(String),
}


pub type ConductorApiResult<T> = Result<T, ConductorApiError>;

    // pub type ConductorApiResult<I: CellConductorInterfaceT, T> =
//     Result<T, ConductorApiError< <I::Cell as CellT>::Error>>;

// pub type InterfaceConductorResult<I: CellConductorInterfaceT, T> =
//     Result<T, ConductorApiError<<I::Conductor as ConductorT>::Error>>;

// #[derive(Error, Debug)]
// pub(crate) enum CellError {
//     // #[error("error dealing with workspace state: {0}")]
//     // DatabaseError(#[from] DatabaseError),
// }

// pub(crate) type CellResult<T> = Result<T, CellError>;

// #[derive(Error, Debug)]
// pub(crate) enum ConductorError {
//     // #[error("error dealing with workspace state: {0}")]
// // DatabaseError(#[from] DatabaseError),
// }

// pub(crate) type ConductorResult<T> = Result<T, ConductorError>;
