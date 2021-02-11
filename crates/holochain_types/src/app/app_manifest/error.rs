use thiserror::Error;

use crate::prelude::CellNick;

#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum AppManifestError {
    #[error("Missing required field in app manifest: {0}")]
    MissingField(String),

    #[error("Invalid manifest for cell nick '{0}': Using strategy 'disabled' with clone_limit == 0 is pointless")]
    InvalidStrategyDisabled(CellNick),
}

pub type AppManifestResult<T> = Result<T, AppManifestError>;
