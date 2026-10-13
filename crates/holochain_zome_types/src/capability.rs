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

use crate::cell::CellId;
use holo_hash::ActionHash;
use holochain_integrity_types::prelude::{CapGrant, DesensitizedCapGrant};
use holochain_timestamp::Timestamp;
use serde::{Deserialize, Serialize};

mod grant;
pub use grant::*;
use holochain_integrity_types::capability::{Capability, GrantConstraint, ZomeCallGrant};

/// Parameters for granting a zome call capability.
#[derive(Debug, Deserialize, Serialize)]
pub struct GrantZomeCallCapabilityPayload {
    /// Cell for which to authorize the capability.
    pub cell_id: CellId,
    /// The grant to commit, specifying the capability to grant and the constraint
    /// under which it may be used.
    pub cap_grant: GrantZomeCallCapabilityGrant,
}

/// Part of a [`GrantZomeCallCapabilityPayload`] payload to be translated to a [`CapGrant`] internally.
///
/// Decoding also accepts the flat `{ tag, access, functions }` layout that clients built
/// for Holochain 0.7.0 send.
#[derive(Debug, Serialize)]
pub struct GrantZomeCallCapabilityGrant {
    /// A string by which to later query for saved grants.
    /// This does not need to be unique within a source chain.
    pub tag: String,
    /// Specifies who may claim this capability, and by what means
    pub constraint: GrantConstraint,
    /// Grant access to zome calls with a [`ZomeCallGrant`].
    pub grant: ZomeCallGrant,
}

/// Every wire layout a [`GrantZomeCallCapabilityGrant`] can be decoded from.
#[derive(Deserialize)]
struct GrantZomeCallCapabilityGrantWire {
    tag: String,
    #[serde(default)]
    constraint: Option<GrantConstraint>,
    #[serde(default)]
    access: Option<GrantConstraint>,
    #[serde(default)]
    grant: Option<ZomeCallGrant>,
    #[serde(default)]
    functions: Option<holochain_integrity_types::capability::GrantedFunctions>,
}

impl<'de> Deserialize<'de> for GrantZomeCallCapabilityGrant {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = GrantZomeCallCapabilityGrantWire::deserialize(deserializer)?;
        let constraint = match (wire.constraint, wire.access) {
            (Some(constraint), _) => constraint,
            (None, Some(access)) => access,
            (None, None) => return Err(serde::de::Error::missing_field("constraint")),
        };
        let grant = match (wire.grant, wire.functions) {
            (Some(grant), None) => grant,
            (None, Some(functions)) => ZomeCallGrant { functions },
            (None, None) => return Err(serde::de::Error::missing_field("grant")),
            (Some(_), Some(_)) => {
                return Err(serde::de::Error::custom(
                    "a zome call grant carries either `grant` or `functions`, not both",
                ))
            }
        };
        Ok(GrantZomeCallCapabilityGrant {
            tag: wire.tag,
            constraint,
            grant,
        })
    }
}

impl From<GrantZomeCallCapabilityGrant> for CapGrant {
    fn from(grant: GrantZomeCallCapabilityGrant) -> Self {
        CapGrant {
            tag: grant.tag,
            constraint: grant.constraint,
            capability: Capability::ZomeCall(grant.grant),
        }
    }
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
    /// The granted capability and its constraint, with secrets removed.
    pub cap_grant: DesensitizedCapGrant,
    /// The action hash of the grant.
    pub action_hash: ActionHash,
    /// Time the capability grant was created.
    pub created_at: Timestamp,
    /// Timestamp of capability revocation if revoked.
    pub revoked_at: Option<Timestamp>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_integrity_types::capability::GrantedFunctions;

    fn expected() -> CapGrant {
        CapGrant::new_zome_call_grant(
            "tag".into(),
            GrantConstraint::Unrestricted,
            GrantedFunctions::All,
        )
    }

    #[test]
    fn flat_0_7_0_grant_payload_decodes() {
        #[derive(Serialize, Debug)]
        struct Legacy {
            tag: String,
            access: GrantConstraint,
            functions: GrantedFunctions,
        }
        let legacy = Legacy {
            tag: "tag".into(),
            access: GrantConstraint::Unrestricted,
            functions: GrantedFunctions::All,
        };
        let grant: GrantZomeCallCapabilityGrant = holochain_serialized_bytes::decode(
            &holochain_serialized_bytes::encode(&legacy).unwrap(),
        )
        .unwrap();
        assert_eq!(CapGrant::from(grant), expected());
    }

    #[test]
    fn nested_grant_payload_decodes() {
        #[derive(Serialize, Debug)]
        struct Nested {
            tag: String,
            constraint: GrantConstraint,
            grant: ZomeCallGrant,
        }
        let nested = Nested {
            tag: "tag".into(),
            constraint: GrantConstraint::Unrestricted,
            grant: ZomeCallGrant {
                functions: GrantedFunctions::All,
            },
        };
        let grant: GrantZomeCallCapabilityGrant = holochain_serialized_bytes::decode(
            &holochain_serialized_bytes::encode(&nested).unwrap(),
        )
        .unwrap();
        assert_eq!(CapGrant::from(grant), expected());
    }
}
