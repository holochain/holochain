//! Just enough to get us rolling for now.
//! Not the intended final struct for Errors.

use holochain_json_api::error::JsonError;
use holochain_persistence_api::error::PersistenceError;
use lib3h_crypto_api::CryptoError;
use serde_json::Error as SerdeError;
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SkunkError {
    Todo(String),
    NoneError,
    IoError(#[from] std::io::Error),
    HcidError(#[from] hcid::HcidError),
    SerdeError(#[from] SerdeError),
    JsonError(#[from] JsonError),
    CryptoError(#[from] CryptoError),
    PersistenceError(#[from] PersistenceError),
    Base64DecodeError(#[from] base64::DecodeError),
    Utf8Error(#[from] std::str::Utf8Error),
    LocksmithError(#[from] holochain_locksmith::LocksmithError),
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
            (PersistenceError(a), PersistenceError(b)) => a == b,
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
    pub fn new<S: Into<String>>(s: S) -> Self {
        SkunkError::Todo(s.into())
    }
}

pub type SkunkResult<T> = Result<T, SkunkError>;
