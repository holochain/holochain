//! The Signature type is defined here. They are used in ChainHeaders as
//! a way of providing cryptographically verifiable proof of a given agent
//! as having been the author of a given data entry.

use crate::prelude::*;
use holo_hash::AgentHash;

/// Provenance is a tuple of initiating agent public key and signature of some item being signed
/// this type is used in headers and in capability requests where the item being signed
/// is implicitly known by context
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq, SerializedBytes)]
pub struct Provenance(AgentHash, Signature);

impl Provenance {
    /// Creates a new provenance instance with source typically
    /// being an agent address (public key) and the signature
    /// some signed data using the private key associated with
    /// the public key.
    pub fn new(source: AgentHash, signature: Signature) -> Self {
        Provenance(source, signature)
    }

    /// who generated this signature
    pub fn source(&self) -> AgentHash {
        self.0.clone()
    }

    /// the actual signature data
    pub fn signature(&self) -> Signature {
        self.1.clone()
    }
}
