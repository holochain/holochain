//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use futures::future::FutureExt;
use holo_hash::*;
use holo_hash_core::hash_type;
use holochain_serialized_bytes::prelude::*;
pub use holochain_zome_types::entry::Entry;
use must_future::MustBoxFuture;

pub type EntryHashed = HoloHashed<Entry>;

impl HashableContent for Entry {
    type HashType = hash_type::Entry;

    fn hash_type(&self) -> Self::HashType {
        match self {
            Entry::Agent(_) => hash_type::Entry::Agent,
            _ => hash_type::Entry::Content,
        }
    }
}
