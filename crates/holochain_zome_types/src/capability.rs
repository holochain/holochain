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

mod grant;
pub use grant::*;

use holo_hash::{AgentPubKey, ZomeCallSigningKey};
pub use holochain_integrity_types::capability::*;
use serde::{Deserialize, Serialize};

use crate::CellId;

/// Parameters for authorizing a zome call signing key.
#[derive(Debug, Deserialize, Serialize)]
pub struct AuthorizeZomeCallSigningKeyPayload {
    /// Agent for whom to authorize the signing key.
    pub agent_pub_key: AgentPubKey,
    /// Cell for which to authorize the signing key.
    pub cell_id: CellId,
    /// Zomes and functions for which to authorize the signing key.
    pub functions: GrantedFunctions,
    /// The public key of the signing key pair.
    pub signing_key: ZomeCallSigningKey,
    /// The cap secret for the cap grant.
    pub cap_secret: CapSecret,
}
