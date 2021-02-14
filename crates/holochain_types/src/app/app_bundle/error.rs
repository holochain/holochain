use mr_bundle::error::MrBundleError;

use crate::prelude::{AppManifestError, CellNick, DnaError};

/// Errors occurring while installing an AppBundle
#[derive(thiserror::Error, Debug)]
pub enum AppBundleError {
    // #[error(transparent)]
    // CellResolutionFailure(#[from] CellResolutionFailure),
    #[error("Could not resolve the cell slot '{0}'")]
    CellResolutionFailure(CellNick),

    #[error(transparent)]
    AppManifestError(#[from] AppManifestError),

    #[error(transparent)]
    DnaError(#[from] DnaError),

    #[error(transparent)]
    MrBundleError(#[from] MrBundleError),

    #[error(transparent)]
    FfsIoError(#[from] ffs::IoError),
}

pub type AppBundleResult<T> = Result<T, AppBundleError>;

// #[derive(thiserror::Error, Debug, shrinkwraprs::Shrinkwrap)]
// #[shrinkwrap(mutable, unsafe_ignore_mutability)]
// #[error("The following cell slots could not be resolved: {0:?}")]
// pub struct CellResolutionFailure(Vec<CellNick>);
