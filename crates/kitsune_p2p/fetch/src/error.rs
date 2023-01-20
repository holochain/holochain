#![allow(missing_docs)]

use thiserror::Error;

use crate::FetchKey;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("Key not present in the queue: {0:?}")]
    KeyMissing(FetchKey),
}

/// Kitsune Fetch Result
pub type FetchResult<T> = Result<T, FetchError>;
