use super::CapSecret;
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;

/// System entry to hold a capability token claim for use as a caller.
/// Stored by a claimant so they can remember what's necessary to exercise
/// this capability by sending the secret to the grantor.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, SerializedBytes)]
pub struct CapClaim {
    /// A string by which to later query for saved claims.
    /// This does not need to be unique within a source chain.
    pub tag: String,
    /// AgentPubKey of agent who authored the corresponding CapGrant.
    pub grantor: AgentPubKey,
    /// The secret needed to exercise this capability.
    /// This is the only bit sent over the wire to attempt a remote call.
    /// Note that the grantor may have revoked the corresponding grant since we received the claim
    /// so claims are only ever a 'best effort' basis.
    pub secret: CapSecret,
}

impl CapClaim {
    /// Constructor.
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
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Access the grantor
    pub fn grantor(&self) -> &AgentPubKey {
        &self.grantor
    }
}
