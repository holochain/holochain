//! Holochain DnaError type.

use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

/// Holochain DnaError type.
#[derive(Clone, Debug, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub enum DnaError {
    /// ZomeNotFound
    ZomeNotFound(String),

    /// TraitNotFound
    TraitNotFound(String),

    /// ZomeFunctionNotFound
    ZomeFunctionNotFound(String),

    /// we attempted to verify the Dna and a zome had no code in it
    ZomeNoCode(String),
}

impl Error for DnaError {}

impl fmt::Display for DnaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg: String = match self {
            DnaError::ZomeNotFound(err_msg) => err_msg.into(),
            DnaError::TraitNotFound(err_msg) => err_msg.into(),
            DnaError::ZomeFunctionNotFound(err_msg) => err_msg.into(),
            DnaError::ZomeNoCode(zome_name) => format!("Zome {} has no code!", zome_name),
        };
        write!(f, "{}", msg)
    }
}
