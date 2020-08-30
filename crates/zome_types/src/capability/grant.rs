use super::CapSecret;
use crate::zome::ZomeName;
use holo_hash::*;
use holochain_serialized_bytes::SerializedBytes;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// System entry to hold a capabilities granted by the callee
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum CapGrant {
    /// Grants the capability of writing to the source chain for this agent key.
    /// This grant is provided by the `Entry::Agent` entry on the source chain.
    Authorship(AgentPubKey),

    /// General capability for giving fine grained access to zome functions
    /// and/or private data
    ZomeCall(ZomeCallCapGrant),
}

#[derive(Default, PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
/// @todo the ability to forcibly curry payloads into functions that are called with a claim
pub struct CurryPayloads(pub BTreeMap<GrantedFunction, SerializedBytes>);

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
/// The payload for the ZomeCall capability grant.
/// This data is committed to the source chain as a private entry.
pub struct ZomeCallCapGrant {
    /// A string by which to later query for saved grants.
    /// This does not need to be unique within a source chain.
    pub tag: String,
    /// Specifies who may claim this capability, and by what means
    pub access: CapAccess,
    /// Set of functions to which this capability grants ZomeCall access
    pub functions: GrantedFunctions,
    /// the payloads to curry to the functions
    pub curry_payloads: CurryPayloads,
}

impl ZomeCallCapGrant {
    /// Constructor
    pub fn new(
        tag: String,
        access: CapAccess,
        functions: GrantedFunctions,
        curry_payloads: CurryPayloads,
    ) -> Self {
        Self {
            tag,
            access,
            functions,
            curry_payloads,
        }
    }
}

impl From<ZomeCallCapGrant> for CapGrant {
    /// Create a new ZomeCall capability grant
    fn from(zccg: ZomeCallCapGrant) -> Self {
        CapGrant::ZomeCall(zccg)
    }
}

impl CapGrant {
    /// Check if a tag matches this grant.
    /// An Authorship grant has no tag, thus will never match any tag
    pub fn tag_matches(&self, query: &str) -> bool {
        match self {
            CapGrant::Authorship(_) => false,
            CapGrant::ZomeCall(ZomeCallCapGrant { tag, .. }) => tag == query,
        }
    }

    /// Get the CapAccess data in order to check authorization
    pub fn access(&self) -> CapAccess {
        match self {
            CapGrant::Authorship(agent_pubkey) => CapAccess::Assigned {
                // there is nothing meaningful about a self-assigned secret so we might as well
                // zero it out to (hopefully) make it very clear that this has a different security
                // and access model (i.e. that the caller of the function is the current agent).
                secret: ().into(),
                assignees: [agent_pubkey.clone()].iter().cloned().collect(),
            },
            CapGrant::ZomeCall(ZomeCallCapGrant { access, .. }) => access.clone(),
        }
    }
}

/// Represents access requirements for capability grants
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum CapAccess {
    /// No restriction: accessible by anyone
    Unrestricted,
    /// Accessible by anyone who can provide the secret
    Transferable {
        /// The secret
        secret: CapSecret,
    },
    /// Accessible by anyone in the list of assignees who possesses the secret
    Assigned {
        /// The secret
        secret: CapSecret,
        /// The set of agents who may exercise this grant
        assignees: HashSet<AgentPubKey>,
    },
}

impl Default for CapAccess {
    fn default() -> Self {
        Self::Unrestricted
    }
}

impl From<()> for CapAccess {
    fn from(_: ()) -> Self {
        Self::Unrestricted
    }
}

impl From<CapSecret> for CapAccess {
    fn from(secret: CapSecret) -> Self {
        Self::Transferable { secret }
    }
}

impl From<(CapSecret, HashSet<AgentPubKey>)> for CapAccess {
    fn from((secret, assignees): (CapSecret, HashSet<AgentPubKey>)) -> Self {
        Self::Assigned { secret, assignees }
    }
}

impl From<(CapSecret, AgentPubKey)> for CapAccess {
    fn from((secret, assignee): (CapSecret, AgentPubKey)) -> Self {
        let mut assignees = HashSet::new();
        assignees.insert(assignee);
        Self::from((secret, assignees))
    }
}

impl CapAccess {
    /// Check if access is granted given the inputs
    pub fn is_authorized(&self, agent_key: &AgentPubKey, maybe_secret: Option<&CapSecret>) -> bool {
        match self {
            CapAccess::Unrestricted => true,
            CapAccess::Transferable { secret } => Some(secret) == maybe_secret,
            CapAccess::Assigned { secret, assignees } => {
                Some(secret) == maybe_secret && assignees.contains(agent_key)
            }
        }
    }

    /// If this CapAccess has a secret, get it
    pub fn secret(&self) -> Option<&CapSecret> {
        match self {
            CapAccess::Transferable { secret } | CapAccess::Assigned { secret, .. } => Some(secret),
            CapAccess::Unrestricted => None,
        }
    }
}

/// a single function name
pub type FunctionName = String;
/// a single zome/function pair
pub type GrantedFunction = (ZomeName, FunctionName);
/// A collection of zome/function pairs
pub type GrantedFunctions = HashSet<GrantedFunction>;
