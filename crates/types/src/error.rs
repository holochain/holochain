//! Just enough to get us rolling for now.
//! Not the intended final struct for Errors.

use crate::prelude::*;
use holochain_json_api::error::JsonError;
use holochain_persistence_api::error::PersistenceError;
use serde_json::Error as SerdeError;
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SkunkError {
    Todo(String),
    IoError(#[from] std::io::Error),
    HcidError(#[from] hcid::HcidError),
    SerdeError(#[from] SerdeError),
    JsonError(#[from] JsonError),
    PersistenceError(#[from] PersistenceError),
    Base64DecodeError(#[from] base64::DecodeError),
}

impl fmt::Display for SkunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkunkError::Todo(reason) => write!(f, "{}", reason),
            _ => write!(f, "{:?}", self),
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
