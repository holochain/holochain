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
//!   exercise the capability
//! - Assigned: Like Transferable, but there is a list of approved AgentPubKeys,
//!   and requests from any other agents are ignored.
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

use serde::{Deserialize, Serialize};

use crate::prelude::*;

mod grant;
pub use grant::*;

pub use holochain_integrity_types::capability::*;

/// Parameters for granting a zome call capability.
#[derive(Debug, Deserialize, Serialize)]
pub struct GrantZomeCallCapabilityPayload {
    /// Cell for which to authorize the capability.
    pub cell_id: CellId,
    /// Specifies the capability, consisting of zomes and functions to allow
    /// signing for as well as access level, secret and assignees.
    pub cap_grant: ZomeCallCapGrant,
}

/// A list which map a cell ID to their capability grant information.
///
/// NOTE: while a map would have been more appropriate, we use a vector here
/// because it is problematic with msgpack encoding.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppCapGrantInfo(pub Vec<(CellId, Vec<CapGrantInfo>)>);

/// Information about a capability grant.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CapGrantInfo {
    /// Specifies the capability, consisting of zomes and functions to allow
    /// signing for as well as access level, secret and assignees.
    pub cap_grant: DesensitizedZomeCallCapGrant,
    /// The action hash of the grant.
    pub action_hash: ActionHash,
    /// Time the capability grant was created.
    pub created_at: Timestamp,
    /// Timestamp of capability revocation if revoked.
    pub revoked_at: Option<Timestamp>,
}
