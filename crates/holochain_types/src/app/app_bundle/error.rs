use mr_bundle::error::MrBundleError;

use crate::prelude::{AppManifestError, CellNick, DnaError};

/// Errors occurring while installing an AppBundle
#[derive(thiserror::Error, Debug)]
pub enum AppBundleError {
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
