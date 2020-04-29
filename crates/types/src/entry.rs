//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::address::EntryAddress;
use holo_hash::*;
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
    /// The Agent system entry, the second entry of every source chain,
    /// which grants authoring capability for this agent. (Name TBD)
    Agent(AgentPubKey),
    /// The application entry data for entries that aren't system created entries
    App(SerializedBytes),
    /// The capability claim system entry which allows committing a granted permission
    /// for later use
    CapTokenClaim(CapTokenClaim),
    /// The capability grant system entry which allows granting of application defined
    /// capabilities
    CapTokenGrant(CapTokenGrant),
}

impl Entry {
    /// Get the EntryAddress of this entry
    // FIXME: use async with_data, or consider wrapper type
    // https://github.com/Holo-Host/holochain-2020/pull/86#discussion_r413226841
    pub fn entry_address(&self) -> EntryAddress {
        match self {
            Entry::Agent(key) => EntryAddress::Agent(key.to_owned()),
            Entry::App(serialized_bytes) => {
                EntryAddress::Entry(EntryHash::with_data_sync(&serialized_bytes.bytes()))
            }
            Entry::CapTokenClaim(claim) => {
                // TODO fix unwrap
                let serialized_bytes: SerializedBytes = claim.try_into().unwrap();
                EntryAddress::Entry(EntryHash::with_data_sync(serialized_bytes.bytes()))
            }
            Entry::CapTokenGrant(grant) => {
                // TODO fix unwrap
                let serialized_bytes: SerializedBytes = grant.try_into().unwrap();
                EntryAddress::Entry(EntryHash::with_data_sync(&serialized_bytes.bytes()))
            }
        }
    }
}
