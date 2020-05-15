//! Capability Grants and Claims
//!
//! This module provides a custom system for defining application-specific
//! capabilities, and allowing others to access those capabilities in a
//! fine-grained manner.
//!
//! TODO: write more

use derive_more::{From, Into};
use serde::{Deserialize, Serialize};

mod claim;
mod grant;
pub use claim::*;
pub use grant::*;

/// A CapSecret is a secret that is used to claim this set of permissions
/// It is a random, unique identifier for this capability,
/// except in the case of public capabilities, in which case it is merely unique.
/// A capability CAN be updated (replaced with a new one) with the same secret.
/// NB for review: previously the secret was the address of the entry,
/// but this is unsafe due to the small search space for a valid CapGrant.
#[derive(From, Into, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CapSecret(String);

impl CapSecret {
    /// Creates a new unique secret from randomness.
    pub fn random() -> Self {
        Self(nanoid::nanoid!())
    }

    /// Creates a secret from a known string.
    pub fn from_string<S: Into<String>>(s: S) -> Self {
        Self(s.into())
    }
}
