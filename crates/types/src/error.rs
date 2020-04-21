//! SkunkError should go away as soon as possible.
//! It is a catch-all for the various error types produced by code in the
//! previous Holochain version, as well as a rough replacement for
//! HolochainError in that version.
//! As we decide which previous code to use, we should port those error types
//! over to the appropriate error type in this crate.

use std::fmt;
use thiserror::Error;

/// Holochain high-level error type
/// TODO - Stop calling this "Skunk"
#[allow(missing_docs)] // these are self explanitory
#[derive(Error, Debug)]
pub enum SkunkError {
    Todo(String),
}

impl fmt::Display for SkunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkunkError::Todo(reason) => write!(f, "{}", reason),
        }
    }
}

impl PartialEq for SkunkError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
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
