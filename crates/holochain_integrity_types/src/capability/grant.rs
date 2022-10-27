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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

/// Tag for an agent's signing key cap grant
pub const SIGNING_KEY_TAG: &str = "signing_key";

impl CapGrant {
    /// Given a grant, is it valid in isolation?
    /// In a world of CRUD, some new entry might update or delete an existing one, but we can check
    /// if a grant is valid in a standalone way.
    pub fn is_valid(
        &self,
        check_function: &GrantedFunction,
        check_agent: &AgentPubKey,
        check_secret: Option<&CapSecret>,
    ) -> bool {
        match self {
            // Grant is always valid if the author matches the check agent.
            CapGrant::ChainAuthor(author) => author == check_agent,
            // Otherwise we need to do more work…
            CapGrant::RemoteAgent(ZomeCallCapGrant {
                access, functions, ..
            }) => {
                // The checked function needs to be in the grant…
                functions.contains(check_function)
                // The agent needs to be valid…
                && match access {
                    // The grant is assigned so the agent needs to match…
                    CapAccess::Assigned { assignees, .. } => assignees.contains(check_agent),
                    // The grant has no assignees so is always valid…
                    _ => true,
                }
                // The secret needs to match…
                && match access {
                    // Unless the extern is unrestricted.
                    CapAccess::Unrestricted => true,
                    // note the PartialEq implementation is constant time for secrets
                    CapAccess::Transferable { secret, .. } => check_secret.map(|given| secret == given).unwrap_or(false),
                    CapAccess::Assigned { secret, .. } => check_secret.map(|given| secret == given).unwrap_or(false),
                }
            }
        }
    }
}

/// Represents access requirements for capability grants.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

/// a single zome/function pair
pub type GrantedFunction = (ZomeName, FunctionName);
/// A collection of zome/function pairs
pub type GrantedFunctions = BTreeSet<GrantedFunction>;
