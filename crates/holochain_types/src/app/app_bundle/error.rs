use holochain_util::ffs;
use mr_bundle::error::MrBundleError;

use crate::prelude::{AppManifestError, DnaError, RoleName};

/// Errors occurring while installing an AppBundle
#[derive(thiserror::Error, Debug)]
pub enum AppBundleError {
    #[error("Could not resolve the app role '{0}'. Detail: {1}")]
    CellResolutionFailure(RoleName, String),

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
