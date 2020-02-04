//! Just enough to get us rolling for now.
//! Definitely not even close to the intended final struct for Errors.

use holochain_json_api::error::JsonError;
use holochain_persistence_api::error::PersistenceError;
use serde_json::Error as SerdeError;
use std::fmt;

#[derive(Debug, PartialEq, Clone)]
pub struct SkunkError(String);

impl fmt::Display for SkunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SkunkError {}

impl From<String> for SkunkError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl SkunkError {
    pub fn new<S: Into<String>>(s: S) -> Self {
        Self(s.into())
    }
}

pub type SkunkResult<T> = Result<T, SkunkError>;

impl From<hcid::HcidError> for SkunkError {
    fn from(error: hcid::HcidError) -> Self {
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

impl From<base64::DecodeError> for SkunkError {
    fn from(error: base64::DecodeError) -> Self {
        SkunkError::new(format!("base64 decode error: {}", error.to_string()))
    }
}


impl From<PersistenceError> for SkunkError {
    fn from(error: PersistenceError) -> Self {
        SkunkError::new(format!("{:?}", error))
    }
}

