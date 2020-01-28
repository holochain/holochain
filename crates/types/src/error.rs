//! Just enough to get us rolling for now.
//! Definitely not even close to the intended final struct for Errors.

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
