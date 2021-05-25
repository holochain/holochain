#![allow(missing_docs)]

use crate::prelude::*;
use holochain_serialized_bytes::SerializedBytesError;
use thiserror::Error;

pub type InlineZomeResult<T> = Result<T, InlineZomeError>;

#[derive(Error, Debug)]
pub enum InlineZomeError {
    #[error("No such InlineZome callback: {0}")]
    NoSuchCallback(FunctionName),

    #[error("Error during host fn call: {0}")]
    HostFnApiError(#[from] HostFnApiError),

    #[error(transparent)]
    SerializationError(#[from] SerializedBytesError),

    #[cfg(any(test, feature = "test_utils"))]
    #[error("test error: {0}")]
    TestError(String),
}
