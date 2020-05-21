//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::{
    capability::{CapClaim, CapGrant, ZomeCallCapGrant},
    composite_hash::EntryHash,
};
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;

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

/// The data type written to the source chain to denote a capability claim
pub type CapClaimEntry = CapClaim;

/// The data type written to the source chain when explicitly granting a capability.
/// NB: this is not simply `CapGrant`, because the `CapGrant::Authorship`
/// grant is already implied by `Entry::Agent`, so that should not be committed
/// to a chain. This is a type alias because if we add other capability types
/// in the future, we may want to include them
pub type CapGrantEntry = ZomeCallCapGrant;

make_hashed_base! {
    Visibility(pub),
    HashedName(EntryHashed),
    ContentType(Entry),
    HashType(EntryHash),
}

impl EntryHashed {
    /// Construct (and hash) a new EntryHashed with given Entry.
    pub async fn with_data(entry: Entry) -> Result<Self, SerializedBytesError> {
        let hash = match &entry {
            Entry::Agent(key) => EntryHash::Agent(key.to_owned()),
            entry => {
                let sb = SerializedBytes::try_from(entry)?;
                EntryHash::Entry(EntryContentHash::with_data(sb.bytes()).await)
            }
        };
        Ok(EntryHashed::with_pre_hashed(entry, hash))
    }
}
