//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use holo_hash_ext::*;
pub use holochain_zome_types::entry::Entry;

/// An Entry paired with its EntryHash
pub type EntryHashed = HoloHashed<Entry>;
