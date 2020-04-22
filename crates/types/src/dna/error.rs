//! Holochain DnaError type.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Holochain DnaError type.
#[derive(Clone, Debug, Error, PartialEq, Hash, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum DnaError {
    /// ZomeNotFound
    #[error("Zome not found: {0}")]
    ZomeNotFound(String),

    /// EmptyZome
    #[error("Zome has no code: {0}")]
    EmptyZome(String),

    /// Invalid
    #[error("DNA is invalid: {0}")]
    Invalid(String),

    /// TraitNotFound
    #[error("Trait not found: {0}")]
    TraitNotFound(String),

    /// ZomeFunctionNotFound
    #[error("Zome function not found: {0}")]
    ZomeFunctionNotFound(String),
}
