use super::{CapAccess, CapSecret};
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use holo_hash::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};

/// The entry for a capability grant.
///
/// This data is committed to the callee's source chain as a private entry. The remote calling
/// agent must provide a secret and we source their pubkey from the active network connection.
/// This must match the strictness of the [`GrantConstraint`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "hdk/capabilities.ts"))]
pub struct CapGrant {
    /// A string by which to later query for saved grants.
    /// This does not need to be unique within a source chain.
    pub tag: String,
    /// Specifies who may claim this capability, and by what means
    pub constraint: GrantConstraint,
    /// The capability to be granted.
    pub capability: Capability,
}

/// The capability to grant in a [`CapGrant`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "hdk/capabilities.ts"))]
pub enum Capability {
    /// Grant access to zome calls with a [`ZomeCallGrant`].
    ZomeCall(ZomeCallGrant),
    /// Grant access to direct signals.
    DirectSignal,
}

/// The inner properties of a [`Capability::ZomeCall`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "hdk/capabilities.ts"))]
pub struct ZomeCallGrant {
    /// Set of functions to which this capability grants ZomeCall access
    pub functions: GrantedFunctions,
}

/// The outbound DTO of a [`CapGrant`].
///
/// [`GrantConstraint`] secrets are omitted, access types and assignees are provided under
/// [`GrantConstraintInfo`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "hdk/capabilities.ts"))]
pub struct DesensitizedCapGrant {
    /// A string by which to later query for saved grants.
    /// This does not need to be unique within a source chain.
    pub tag: String,
    /// Equivalent to the [`GrantConstraint`] type but with secrets removed.
    pub constraint: GrantConstraintInfo,
    /// The capability that is granted.
    pub capability: Capability,
}

impl From<CapGrant> for DesensitizedCapGrant {
    fn from(grant: CapGrant) -> Self {
        DesensitizedCapGrant {
            tag: grant.tag,
            constraint: GrantConstraintInfo {
                access_type: grant.constraint.as_variant_string().to_string(),
                assignees: match &grant.constraint {
                    GrantConstraint::Assigned { assignees, .. } => Some(assignees.clone()),
                    _ => None,
                },
            },
            capability: grant.capability,
        }
    }
}

impl CapGrant {
    /// Constructor
    pub fn new_zome_call_grant(
        tag: String,
        constraint: GrantConstraint,
        functions: GrantedFunctions,
    ) -> Self {
        Self {
            tag,
            constraint,
            capability: Capability::ZomeCall(ZomeCallGrant { functions }),
        }
    }
}

impl From<CapGrant> for CapAccess {
    fn from(grant: CapGrant) -> Self {
        CapAccess::RemoteAgent(grant)
    }
}

impl CapAccess {
    /// Given a grant, is it valid in isolation for a zome call?
    ///
    /// In a world of CRUD, some new entry might update or delete an existing one, but we can check
    /// if a grant is valid in a standalone way.
    pub fn is_valid_for_zome_call(
        &self,
        given_function: &GrantedFunction,
        given_agent: &AgentPubKey,
        given_secret: Option<&CapSecret>,
    ) -> bool {
        match self {
            // Grant is always valid if the author matches the check agent.
            CapAccess::ChainAuthor(author) => author == given_agent,
            // Otherwise we need to do more work…
            CapAccess::RemoteAgent(CapGrant {
                constraint,
                capability: Capability::ZomeCall(ZomeCallGrant { functions }),
                ..
            }) => {
                // The checked function needs to be in the grant…
                let granted = match functions {
                    GrantedFunctions::All => true,
                    GrantedFunctions::Listed(fns) => fns.contains(given_function),
                };

                granted && constraint.permits(given_agent, given_secret)
            }
            _ => false,
        }
    }

    /// Given a grant, is it valid for a direct signal?
    pub fn is_valid_for_direct_signal(
        &self,
        given_agent: &AgentPubKey,
        given_secret: Option<&CapSecret>,
    ) -> bool {
        match self {
            CapAccess::RemoteAgent(CapGrant {
                constraint,
                capability: Capability::DirectSignal,
                ..
            }) => constraint.permits(given_agent, given_secret),
            _ => false,
        }
    }
}

/// Represents the constraints on the use of a capability grant.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "hdk/capabilities.ts"))]
pub enum GrantConstraint {
    /// No restriction: callable by anyone.
    Unrestricted,
    /// Callable by anyone who can provide the secret.
    Transferable {
        /// The secret.
        secret: CapSecret,
    },
    /// Callable by anyone in the list of assignees who possesses the secret.
    Assigned {
        /// The secret.
        secret: CapSecret,
        /// Agents who can use this grant.
        assignees: BTreeSet<AgentPubKey>,
    },
}

impl GrantConstraint {
    /// Check whether the constraint permits the given agent and secret.
    pub fn permits(&self, given_agent: &AgentPubKey, given_secret: Option<&CapSecret>) -> bool {
        let assignee_permitted = match self {
            GrantConstraint::Assigned { assignees, .. } => assignees.contains(given_agent),
            _ => true,
        };

        if !assignee_permitted {
            return false;
        }

        match self {
            GrantConstraint::Unrestricted => true,
            // note the PartialEq implementation is constant time for secrets
            GrantConstraint::Transferable { secret, .. } => {
                given_secret.map(|given| secret == given).unwrap_or(false)
            }
            GrantConstraint::Assigned { secret, .. } => {
                given_secret.map(|given| secret == given).unwrap_or(false)
            }
        }
    }
}

/// Implements ().into() shorthand for [`GrantConstraint::Unrestricted`]
impl From<()> for GrantConstraint {
    fn from(_: ()) -> Self {
        Self::Unrestricted
    }
}

/// Implements secret.into() shorthand for [`GrantConstraint::Transferable`]
impl From<CapSecret> for GrantConstraint {
    fn from(secret: CapSecret) -> Self {
        Self::Transferable { secret }
    }
}

/// Implements (secret, assignees).into() shorthand for [`GrantConstraint::Assigned`]
impl From<(CapSecret, BTreeSet<AgentPubKey>)> for GrantConstraint {
    fn from((secret, assignees): (CapSecret, BTreeSet<AgentPubKey>)) -> Self {
        Self::Assigned { secret, assignees }
    }
}

/// Implements (secret, agent_pub_key).into() shorthand for [`GrantConstraint::Assigned`].
impl From<(CapSecret, AgentPubKey)> for GrantConstraint {
    fn from((secret, assignee): (CapSecret, AgentPubKey)) -> Self {
        let mut assignees = BTreeSet::new();
        assignees.insert(assignee);
        Self::from((secret, assignees))
    }
}

impl GrantConstraint {
    /// Return variant denominator as string slice
    pub fn as_variant_string(&self) -> &str {
        match self {
            GrantConstraint::Unrestricted => "unrestricted",
            GrantConstraint::Transferable { .. } => "transferable",
            GrantConstraint::Assigned { .. } => "assigned",
        }
    }
}

/// Represents [`GrantConstraint`] info for capability grants, with secrets removed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "hdk/capabilities.ts"))]
pub struct GrantConstraintInfo {
    /// The access type.
    access_type: String,
    /// Agents who can use this grant.
    assignees: Option<BTreeSet<AgentPubKey>>,
}

/// a single zome/function pair
pub type GrantedFunction = (ZomeName, FunctionName);

#[cfg(feature = "ts_rs")]
holo_hash::ts_alias!(
    GrantedFunctionTs,
    "GrantedFunction",
    "[ZomeName, FunctionName]",
    "hdk/capabilities.ts",
    deps: [ZomeName, FunctionName]
);

/// A collection of zome/function pairs
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "hdk/capabilities.ts"))]
pub enum GrantedFunctions {
    /// grant all zomes all functions
    All,
    /// grant to specified zomes and functions
    Listed(
        #[cfg_attr(
            feature = "ts_rs",
            ts(as = "std::collections::HashSet<GrantedFunctionTs>")
        )]
        HashSet<GrantedFunction>,
    ),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_grant_is_valid() {
        let agent1 = AgentPubKey::from_raw_36(vec![1; 36]);
        let agent2 = AgentPubKey::from_raw_36(vec![2; 36]);
        let assignees: BTreeSet<_> = [agent1.clone()].into_iter().collect();
        let secret: CapSecret = [1; 64].into();
        let secret_wrong: CapSecret = [2; 64].into();
        let tag = "tag".to_string();

        let g1: CapAccess = CapGrant {
            tag: tag.clone(),
            constraint: GrantConstraint::Transferable { secret },
            capability: Capability::ZomeCall(ZomeCallGrant {
                functions: GrantedFunctions::All,
            }),
        }
        .into();

        let g2: CapAccess = CapGrant {
            tag: tag.clone(),
            constraint: GrantConstraint::Assigned {
                secret,
                assignees: assignees.clone(),
            },
            capability: Capability::ZomeCall(ZomeCallGrant {
                functions: GrantedFunctions::All,
            }),
        }
        .into();

        assert!(g1.is_valid_for_zome_call(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            Some(&secret),
        ));

        assert!(g1.is_valid_for_zome_call(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent2,
            Some(&secret),
        ));

        assert!(!g1.is_valid_for_zome_call(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            Some(&secret_wrong),
        ));

        assert!(!g1.is_valid_for_zome_call(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            None,
        ));

        assert!(g2.is_valid_for_zome_call(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            Some(&secret),
        ));

        assert!(!g2.is_valid_for_zome_call(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent2,
            Some(&secret),
        ));

        assert!(!g2.is_valid_for_zome_call(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            None,
        ));

        assert!(!g2.is_valid_for_zome_call(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            Some(&secret_wrong),
        ));
    }
}
