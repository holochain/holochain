//! SkunkError should go away as soon as possible.
//! It is a catch-all for the various error types produced by code in the
//! previous Holochain version, as well as a rough replacement for
//! HolochainError in that version.
//! As we decide which previous code to use, we should port those error types
//! over to the appropriate error type in this crate.

use holochain_json_api::error::JsonError;
use lib3h_crypto_api::CryptoError;
use serde_json::Error as SerdeError;
use std::fmt;
use thiserror::Error;

/// Holochain high-level error type
/// TODO - Stop calling this "Skunk"
#[allow(missing_docs)] // these are self explanitory
#[derive(Error, Debug)]
pub enum SkunkError {
    Todo(String),
    NoneError,
    IoError(#[from] std::io::Error),
    HcidError(#[from] hcid::HcidError),
    SerdeError(#[from] SerdeError),
    JsonError(#[from] JsonError),
    CryptoError(#[from] CryptoError),
    Base64DecodeError(#[from] base64::DecodeError),
    Utf8Error(#[from] std::str::Utf8Error),
    // LocksmithError(#[from] holochain_locksmith::LocksmithError),
}

impl fmt::Display for SkunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkunkError::Todo(reason) => write!(f, "{}", reason),
            _ => write!(f, "{:?}", self),
        }
    }
}

impl PartialEq for SkunkError {
    fn eq(&self, other: &Self) -> bool {
        use SkunkError::*;
        match (self, other) {
            (Todo(a), Todo(b)) => a == b,
            (IoError(a), IoError(b)) => a.to_string() == b.to_string(),
            (HcidError(a), HcidError(b)) => a.to_string() == b.to_string(),
            (SerdeError(a), SerdeError(b)) => a.to_string() == b.to_string(),
            (JsonError(a), JsonError(b)) => a == b,
            (CryptoError(a), CryptoError(b)) => a == b,
            (Base64DecodeError(a), Base64DecodeError(b)) => a == b,
            _ => false,
        }
    }
}

impl From<String> for SkunkError {
    fn from(s: String) -> Self {
        SkunkError::Todo(s)
    }
}

impl SkunkError {
    /// Construct an new Error type from something that can be converted to a String
    pub fn new<S: Into<String>>(s: S) -> Self {
        SkunkError::Todo(s.into())
    }
}

/// High-level Holochain Result type
/// TODO - Stop calling this "Skunk"
pub type SkunkResult<T> = Result<T, SkunkError>;
