use super::CapSecret;
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use holo_hash::*;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;

/// Represents a _potentially_ valid access grant to a zome call.
/// Zome call response will be Unauthorized without a valid grant.
///
/// The CapGrant is not always a dedicated entry in the chain.
/// Notably AgentPubKey entries in the current chain act like root access to local zome calls.
///
/// A `CapGrant` is valid if it matches the function, agent and secret for a given zome call.
///
/// See `.is_valid()`
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum CapGrant {
    /// Grants the capability of calling every extern to the calling agent, provided the calling
    /// agent is the local chain author.
    /// This grant is compared to the current `Entry::Agent` entry on the source chain.
    ChainAuthor(AgentPubKey),

    /// Any agent other than the chain author is attempting to call an extern.
    /// The pubkey of the calling agent is secured by the cryptographic handshake at the network
    /// layer and the caller must provide a secret that we check for in a private entry in the
    /// local chain.
    RemoteAgent(ZomeCallCapGrant),
}

impl From<holo_hash::AgentPubKey> for CapGrant {
    fn from(agent_hash: holo_hash::AgentPubKey) -> Self {
        CapGrant::ChainAuthor(agent_hash)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
/// The entry for the ZomeCall capability grant.
/// This data is committed to the callee's source chain as a private entry.
/// The remote calling agent must provide a secret and we source their pubkey from the active
/// network connection. This must match the strictness of the CapAccess.
pub struct ZomeCallCapGrant {
    /// A string by which to later query for saved grants.
    /// This does not need to be unique within a source chain.
    pub tag: String,
    /// Specifies who may claim this capability, and by what means
    pub access: CapAccess,
    /// Set of functions to which this capability grants ZomeCall access
    pub functions: GrantedFunctions,
    // @todo the payloads to curry to the functions
    // pub curry_payloads: CurryPayloads,
}

impl ZomeCallCapGrant {
    /// Constructor
    pub fn new(
        tag: String,
        access: CapAccess,
        functions: GrantedFunctions,
        // @todo curry_payloads: CurryPayloads,
    ) -> Self {
        Self {
            tag,
            access,
            functions,
            // @todo curry_payloads,
        }
    }
}

impl From<ZomeCallCapGrant> for CapGrant {
    /// Create a new ZomeCall capability grant
    fn from(zccg: ZomeCallCapGrant) -> Self {
        CapGrant::RemoteAgent(zccg)
    }
}

impl CapGrant {
    /// Given a grant, is it valid in isolation?
    /// In a world of CRUD, some new entry might update or delete an existing one, but we can check
    /// if a grant is valid in a standalone way.
    pub fn is_valid(
        &self,
        given_function: &GrantedFunction,
        given_agent: &AgentPubKey,
        given_secret: Option<&CapSecret>,
    ) -> bool {
        match self {
            // Grant is always valid if the author matches the check agent.
            CapGrant::ChainAuthor(author) => author == given_agent,
            // Otherwise we need to do more work…
            CapGrant::RemoteAgent(ZomeCallCapGrant {
                access, functions, ..
            }) => {
                // The checked function needs to be in the grant…
                let granted = match functions {
                    GrantedFunctions::All => true,
                    GrantedFunctions::Listed(fns) => fns.contains(given_function),
                };
                granted
                // The agent needs to be valid…
                && match access {
                    // The grant is assigned so the agent needs to match…
                    CapAccess::Assigned { assignees, .. } => assignees.contains(given_agent),
                    // The grant has no assignees so is always valid…
                    _ => true,
                }
                // The secret needs to match…
                && match access {
                    // Unless the extern is unrestricted.
                    CapAccess::Unrestricted => true,
                    // note the PartialEq implementation is constant time for secrets
                    CapAccess::Transferable { secret, .. } => given_secret.map(|given| secret == given).unwrap_or(false),
                    CapAccess::Assigned { secret, .. } => given_secret.map(|given| secret == given).unwrap_or(false),
                }
            }
        }
    }
}

/// Represents access requirements for capability grants.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum CapAccess {
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

/// Implements ().into() shorthand for CapAccess::Unrestricted
impl From<()> for CapAccess {
    fn from(_: ()) -> Self {
        Self::Unrestricted
    }
}

/// Implements secret.into() shorthand for CapAccess::Transferable(secret)
impl From<CapSecret> for CapAccess {
    fn from(secret: CapSecret) -> Self {
        Self::Transferable { secret }
    }
}

/// Implements (secret, assignees).into() shorthand for CapAccess::Assigned { secret, assignees }
impl From<(CapSecret, BTreeSet<AgentPubKey>)> for CapAccess {
    fn from((secret, assignees): (CapSecret, BTreeSet<AgentPubKey>)) -> Self {
        Self::Assigned { secret, assignees }
    }
}

/// Implements (secret, agent_pub_key).into() shorthand for
/// CapAccess::Assigned { secret, assignees: hashset!{ agent } }
impl From<(CapSecret, AgentPubKey)> for CapAccess {
    fn from((secret, assignee): (CapSecret, AgentPubKey)) -> Self {
        let mut assignees = BTreeSet::new();
        assignees.insert(assignee);
        Self::from((secret, assignees))
    }
}

impl CapAccess {
    /// Return variant denominator as string slice
    pub fn as_variant_string(&self) -> &str {
        match self {
            CapAccess::Unrestricted => "unrestricted",
            CapAccess::Transferable { .. } => "transferable",
            CapAccess::Assigned { .. } => "assigned",
        }
    }
}

/// a single zome/function pair
pub type GrantedFunction = (ZomeName, FunctionName);
/// A collection of zome/function pairs

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum GrantedFunctions {
    /// grant all zomes all functions
    All,
    /// grant to specified zomes and functions
    Listed(BTreeSet<GrantedFunction>),
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

        let g1: CapGrant = ZomeCallCapGrant {
            tag: tag.clone(),
            access: CapAccess::Transferable {
                secret: secret.clone(),
            },
            functions: GrantedFunctions::All,
        }
        .into();

        let g2: CapGrant = ZomeCallCapGrant {
            tag: tag.clone(),
            access: CapAccess::Assigned {
                secret: secret.clone(),
                assignees: assignees.clone(),
            },
            functions: GrantedFunctions::All,
        }
        .into();

        assert!(g1.is_valid(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            Some(&secret),
        ));

        assert!(g1.is_valid(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent2,
            Some(&secret),
        ));

        assert!(!g1.is_valid(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            Some(&secret_wrong),
        ));

        assert!(!g1.is_valid(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            None,
        ));

        assert!(g2.is_valid(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            Some(&secret),
        ));

        assert!(!g2.is_valid(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent2,
            Some(&secret),
        ));

        assert!(!g2.is_valid(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            None,
        ));

        assert!(!g2.is_valid(
            &(ZomeName("zome".into()), FunctionName("fn".into())),
            &agent1,
            Some(&secret_wrong),
        ));
    }
}
