//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::{
    address::EntryAddress,
    capability::{CapClaim, CapGrant, ZomeCallCapGrant},
};
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::entry::Entry;

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
    HashType(EntryAddress),
}

impl EntryHashed {
    /// Construct (and hash) a new EntryHashed with given Entry.
    pub async fn with_data(entry: Entry) -> Result<Self, SerializedBytesError> {
        let hash = match &entry {
            Entry::Agent(key) => EntryAddress::Agent(key.to_owned().into()),
            entry => {
                let sb = SerializedBytes::try_from(entry)?;
                EntryAddress::Entry(EntryHash::with_data(sb.bytes()).await)
            }
        };
        Ok(EntryHashed::with_pre_hashed(entry, hash))
    }
}
