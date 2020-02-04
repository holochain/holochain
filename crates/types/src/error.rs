//! Just enough to get us rolling for now.
//! Not the intended final struct for Errors.

use crate::prelude::*;
use holochain_json_api::error::JsonError;
use holochain_persistence_api::error::PersistenceError;
use serde_json::Error as SerdeError;
use std::fmt;

#[derive(Debug, PartialEq, Clone)]
pub enum SkunkError {
    Todo(String),
    IoError(String),
    ConfigError(String),
}

impl fmt::Display for SkunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkunkError::Todo(reason) => write!(f, "{}", reason),
            _ => write!(f, "{:?}", self),
        }
    }
}

impl std::error::Error for SkunkError {}

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

impl From<hcid::HcidError> for SkunkError {
    fn from(error: hcid::HcidError) -> Self {
        SkunkError::new(format!("{:?}", error))
    }
}

impl From<std::io::Error> for SkunkError {
    fn from(error: std::io::Error) -> Self {
        SkunkError::new(format!("{:?}", error))
    }
}

impl From<SerdeError> for SkunkError {
    fn from(error: SerdeError) -> Self {
        SkunkError::new(error.to_string())
    }
}

impl From<JsonError> for SkunkError {
    fn from(error: JsonError) -> Self {
        SkunkError::new(error.to_string())
    }
}

impl From<PersistenceError> for SkunkError {
    fn from(error: PersistenceError) -> Self {
        SkunkError::new(error.to_string())
    }
}

impl From<base64::DecodeError> for SkunkError {
    fn from(error: base64::DecodeError) -> Self {
        SkunkError::new(format!("base64 decode error: {}", error.to_string()))
    }
}
