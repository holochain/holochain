//! An Entry is a unit of data in a Holochain Source Chain.	use holo_hash_core::AgentPubKey;
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::capability::CapClaim;
use crate::capability::CapGrant;
use crate::capability::ZomeCallCapGrant;
use holo_hash_core::{hash_type, AgentPubKey, HashableContent, HashableContentBytes};
use holochain_serialized_bytes::prelude::*;

/// The data type written to the source chain when explicitly granting a capability.
/// NB: this is not simply `CapGrant`, because the `CapGrant::Authorship`
/// grant is already implied by `Entry::Agent`, so that should not be committed
/// to a chain. This is a type alias because if we add other capability types
/// in the future, we may want to include them
pub type CapGrantEntry = ZomeCallCapGrant;

/// The data type written to the source chain to denote a capability claim
pub type CapClaimEntry = CapClaim;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
/// @todo make some options for get
pub struct GetOptions;

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
    pub fn as_cap_grant(&self) -> Option<CapGrant> {
        match self {
            Entry::Agent(key) => Some(CapGrant::Authorship(key.clone())),
            Entry::CapGrant(data) => Some(CapGrant::ZomeCall(data.clone())),
            _ => None,
        }
    }

    /// If this entry represents a capability claim, return a `CapClaim`.
    pub fn as_cap_claim(&self) -> Option<&CapClaim> {
        match self {
            Entry::CapClaim(claim) => Some(claim),
            _ => None,
        }
    }
}

impl HashableContent for Entry {
    type HashType = hash_type::Entry;

    fn hash_type(&self) -> Self::HashType {
        match self {
            Entry::Agent(_) => hash_type::Entry::Agent,
            _ => hash_type::Entry::Content,
        }
    }

    fn hashable_content(&self) -> HashableContentBytes {
        match self {
            Entry::Agent(agent_pubkey) => {
                HashableContentBytes::Prehashed36(agent_pubkey.clone().into_inner())
            }
            entry => HashableContentBytes::Content(
                entry
                    .try_into()
                    .expect("Could not serialize HashableContent"),
            ),
        }
    }
}

// impl HashableContent for &Entry {
//     type HashType = hash_type::Entry;

//     fn hash_type(&self) -> Self::HashType {
//         match self {
//             Entry::Agent(_) => hash_type::Entry::Agent,
//             _ => hash_type::Entry::Content,
//         }
//     }
//     fn hashable_content(&self) -> SerializedBytes {
//         match self {
//             Entry::Agent(agent_pubkey) => todo!(),
//             _ => self.try_into(),
//         }
//         .expect("Could not serialize HashableContent")
//     }
// }
