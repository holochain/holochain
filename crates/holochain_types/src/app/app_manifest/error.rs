use holochain_serialized_bytes::SerializedBytesError;
use thiserror::Error;

use crate::prelude::RoleName;

#[allow(missing_docs)]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppManifestError {
    #[error("Missing required field in app manifest: {0}")]
    MissingField(String),

    #[error("Invalid manifest for app role '{0}': Using strategy 'clone-only' with clone_limit == 0 is pointless")]
    InvalidStrategyCloneOnly(RoleName),

    #[error(transparent)]
    SerializationError(#[from] SerializedBytesError),
}

/// A result that returns a generic type T in case of success and an
/// [`AppManifestError`]` otherwise.
pub type AppManifestResult<T> = Result<T, AppManifestError>;
