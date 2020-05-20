//! An Entry is a unit of data in a Holochain Source Chain.	use holo_hash_core::AgentPubKey;
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use holo_hash_core::AgentPubKey;
use holochain_serialized_bytes::prelude::*;

//TODO move to capabilities module
/// Entry data for a capability claim
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct CapTokenClaim;
/// Entry data for a capability grant
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct CapTokenGrant;

/// Structure holding the entry portion of a chain element.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
#[allow(clippy::large_enum_variant)]
#[serde(tag = "entry_type", content = "entry")]
pub enum Entry {
    /// The `Agent` system entry, the third entry of every source chain,
    /// which grants authoring capability for this agent.
    Agent(AgentPubKey),
    /// The application entry data for entries that aren't system created entries
    App(SerializedBytes),
    /// The capability claim system entry which allows committing a granted permission
    /// for later use
    CapClaim(CapClaimEntry),
    /// The capability grant system entry which allows granting of application defined
    /// capabilities
    CapGrant(CapGrantEntry),
}

impl Entry {
    /// If this entry represents a capability grant, return a `CapGrant`.
    #[allow(dead_code)]
    pub(crate) fn cap_grant(&self) -> Option<CapGrant> {
        match self {
            Entry::Agent(key) => Some(key.clone().into()),
            Entry::CapGrant(data) => Some(data.clone().into()),
            _ => None,
        }
    }
}
