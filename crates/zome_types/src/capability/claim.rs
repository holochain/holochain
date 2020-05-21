use super::CapSecret;
use holo_hash_core::*;
use serde::{Deserialize, Serialize};

/// System entry to hold a capability token claim for use as a caller
/// Stored by a claimant so they can remember what's necessary to exercise
/// this capability by sending the secret to the grantor
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CapClaim {
    /// A string by which to later query for saved claims.
    /// This does not need to be unique within a source chain.
    tag: String,
    /// AgentPubKey of agent who authored the corresponding CapGrant
    grantor: AgentPubKey,
    /// The secret needed to exercise this capability
    secret: CapSecret,
}

impl CapClaim {
    /// Create a new capability claim.
    pub fn new(tag: String, grantor: AgentPubKey, secret: CapSecret) -> Self {
        CapClaim {
            tag,
            grantor,
            secret,
        }
    }

    /// Access the secret.
    pub fn secret(&self) -> &CapSecret {
        &self.secret
    }

    /// Access the tag
    ///
    /// (We may consider changing this to `tag_matches` to match CapGrant)
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Access the grantor
    pub fn grantor(&self) -> &AgentPubKey {
        &self.grantor
    }
}
