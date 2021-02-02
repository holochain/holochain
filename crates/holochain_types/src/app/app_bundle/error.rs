/// Errors occurring while installing an AppBundle
#[derive(thiserror::Error, Debug)]
pub enum AppBundleError {
    // /// Cell was referenced, but is missing from the conductor.
// #[error("Cell was referenced, but is missing from the conductor. CellId: {0:?}")]
// CellMissing(CellId),
}

pub type AppInstallationResult<T> = Result<T, AppBundleError>;
