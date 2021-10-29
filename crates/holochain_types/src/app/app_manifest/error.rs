use thiserror::Error;

use crate::prelude::AppRoleId;

#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum AppManifestError {
    #[error("Missing required field in app manifest: {0}")]
    MissingField(String),

    #[error("Invalid manifest for app role '{0}': Using strategy 'disabled' with clone_limit == 0 is pointless")]
    InvalidStrategyDisabled(AppRoleId),
}

pub type AppManifestResult<T> = Result<T, AppManifestError>;
