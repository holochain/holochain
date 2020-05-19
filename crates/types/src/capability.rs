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

/// A CapSecret is used to claim ability to exercise a capability.
///
/// It is a random, unique identifier for the capability, which is shared by
/// the Grantor to allow access to others.
/// A capability CAN be updated (replaced with a new one) with the same secret.
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
