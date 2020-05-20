use super::CapSecret;
use crate::nucleus::ZomeName;
use derive_more::From;
use holo_hash::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// System entry to hold a capabilities granted by the callee
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, From)]
pub enum CapGrant {
    /// Grants the capability of writing to the source chain for this agent key.
    /// This grant is provided by the `Entry::Agent` entry on the source chain.
    Authorship(AgentPubKey),

    /// General capability for giving fine grained access to zome functions
    /// and/or private data
    ZomeCall(ZomeCallCapGrant),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
/// The payload for the ZomeCall capability grant.
/// This data is committed to the source chain as a private entry.
pub struct ZomeCallCapGrant {
    /// A string by which to later query for saved grants.
    /// This does not need to be unique within a source chain.
    tag: String,
    /// Specifies who may claim this capability, and by what means
    access: CapAccess,
    /// Set of functions to which this capability grants ZomeCall access
    functions: GrantedFunctions,
}

impl CapGrant {
    /// Create a new ZomeCall capability grant
    pub fn zome_call(tag: String, access: CapAccess, functions: GrantedFunctions) -> Self {
        CapGrant::ZomeCall(ZomeCallCapGrant {
            tag,
            access,
            functions,
        })
    }

    /// Check if a tag matches this grant.
    pub fn tag_matches(&self, query: &str) -> bool {
        match self {
            CapGrant::Authorship(agent_pubkey) => agent_pubkey.to_string() == *query,
            CapGrant::ZomeCall(ZomeCallCapGrant { tag, .. }) => tag == query,
        }
    }

    /// Get the CapAccess data in order to check authorization
    pub fn access(&self) -> CapAccess {
        match self {
            CapGrant::Authorship(agent_pubkey) => CapAccess::Assigned {
                secret: agent_pubkey.to_string().into(),
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

impl CapAccess {
    /// Create a new CapAccess::Unrestricted
    pub fn unrestricted() -> Self {
        CapAccess::Unrestricted
    }

    /// Create a new CapAccess::Transferable with random secret
    pub fn transferable() -> Self {
        CapAccess::Transferable {
            secret: CapSecret::random(),
        }
    }

    /// Create a new CapAccess::Assigned with random secret and provided agents
    pub fn assigned(assignees: HashSet<AgentPubKey>) -> Self {
        CapAccess::Assigned {
            secret: CapSecret::random(),
            assignees,
        }
    }

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
}

/// A collection of functions grouped by zome name
/// which are authorized within a capability
pub type GrantedFunctions = BTreeMap<ZomeName, Vec<String>>;
