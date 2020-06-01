//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::composite_hash::EntryHash;
use futures::future::FutureExt;
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
pub use holochain_zome_types::entry::Entry;
use must_future::MustBoxFuture;

make_hashed_base! {
    Visibility(pub),
    HashedName(EntryHashed),
    ContentType(Entry),
    HashType(EntryHash),
}

impl Hashable for EntryHashed {
    /// Construct (and hash) a new EntryHashed with given Entry.
    fn with_data(entry: Entry) -> MustBoxFuture<'static, Result<Self, SerializedBytesError>> {
        async move {
            let hash = match &entry {
                Entry::Agent(key) => EntryHash::Agent(key.to_owned().into()),
                entry => {
                    let sb = SerializedBytes::try_from(entry)?;
                    EntryHash::Entry(
                        EntryContentHash::with_data(UnsafeBytes::from(sb).into()).await,
                    )
                }
            };
            Ok(EntryHashed::with_pre_hashed(entry, hash))
        }
        .boxed()
        .into()
    }
}
