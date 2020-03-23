//! Holochain DnaError type.

use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

/// Holochain DnaError type.
#[derive(Clone, Debug, PartialEq, Hash, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum DnaError {
    /// ZomeNotFound
    ZomeNotFound(String),

    /// TraitNotFound
    TraitNotFound(String),

    /// ZomeFunctionNotFound
    ZomeFunctionNotFound(String),
}

impl Error for DnaError {}

impl fmt::Display for DnaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            DnaError::ZomeNotFound(err_msg) => err_msg,
            DnaError::TraitNotFound(err_msg) => err_msg,
            DnaError::ZomeFunctionNotFound(err_msg) => err_msg,
        };
        write!(f, "{}", msg)
    }
}
