use mr_bundle::error::MrBundleError;

use crate::prelude::AppManifestError;

/// Errors occurring while installing an AppBundle
#[derive(thiserror::Error, Debug)]
pub enum AppBundleError {
    // /// Cell was referenced, but is missing from the conductor.
    // #[error("Cell was referenced, but is missing from the conductor. CellId: {0:?}")]
    // CellMissing(CellId),
    /// Cell was referenced, but is missing from the conductor.

    #[error(transparent)]
    AppManifestError(#[from] AppManifestError),

    #[error(transparent)]
    MrBundleError(#[from] MrBundleError),
}

pub type AppBundleResult<T> = Result<T, AppBundleError>;
