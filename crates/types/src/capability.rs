//! Capability Grants and Claims
//!
//! This module provides a custom system for defining application-specific
//! capabilities, and allowing others to access those capabilities in a
//! fine-grained manner. The Grantor of a capability can receive requests from
//! a Claimant, and if the claim provides the right criteria, the Grantor will
//! perform the task specified by the capability and respond to the Claimant.
//!
//! Capabilities come with three possible degrees of access control:
//! - Unrestricted: anybody can exercise this capability
//! - Transferable: a secret must be provided, but anybody with the secret may
//!     exercise the capability
//! - Assigned: Like Transferable, but there is a list of approved AgentPubKeys,
//!     and requests from any other agents are ignored.
//!
//! Capabilities are declared by a Grantor via a **`CapGrant`**. `CapGrant`s
//! are not directly committed to a source chain, but can be constructed from
//! certain source chain entries. They define a certain bit of functionality,
//! as well as the access controls which determine who may exercise the granted
//! functionality.
//!
//! Capabilites are exercised by other agents via a **`CapClaim`** which they
//! commit to their source chain as a private entry. This struct contains the
//! information needed to refer to the capability as well as the secret needed
//! to send to the Grantor.

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
