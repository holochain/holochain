//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::header::ChainTopOrdering;
use holochain_integrity_types::AppEntryDefName;
use holochain_integrity_types::EntryDefId;
use holochain_integrity_types::ZomeName;
use holochain_serialized_bytes::prelude::*;

mod app_entry_bytes;
pub use app_entry_bytes::*;

pub use holochain_integrity_types::entry::*;

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
/// Either a full [`AppEntryDefLocation`] or one of:
/// - [`EntryType::CapGrant`](crate::prelude::EntryType::CapGrant)
/// - [`EntryType::CapClaim`](crate::prelude::EntryType::CapClaim)
/// Which don't have a location.
pub enum EntryDefLocation {
    /// App defined entries always come from a
    /// specific integrity zomes.
    App(AppEntryDefLocation),
    /// [`crate::EntryDefId::CapClaim`] is committed to and
    /// validated by all integrity zomes in the dna.
    CapClaim,
    /// [`crate::EntryDefId::CapGrant`] is committed to and
    /// validated by all integrity zomes in the dna.
    CapGrant,
}

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
/// A full location of where to find an [`AppEntryDef`](crate::prelude::AppEntryDef)
/// within the dna's zomes.
///
/// The [`ZomeName`] must be unique to the [`DnaDef`](crate::prelude::DnaDef).
/// The [`AppEntryDefName`] must be unique to the zome.
pub struct AppEntryDefLocation {
    /// The name of the integrity zome that defines
    /// and validates the below definition.
    pub zome: ZomeName,
    /// The unique name for this entry definition.
    pub entry: AppEntryDefName,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
/// Options for controlling how get works
pub struct GetOptions {
    /// If this is true the get call will wait for
    /// the latest data before returning.
    /// If it is false you will get whatever is locally
    /// available on this conductor.
    pub strategy: GetStrategy,
}

impl GetOptions {
    /// This will get you the content
    /// with latest metadata if it can
    /// otherwise it will fallback to what
    /// you have cached locally.
    ///
    /// This call is guaranteed to not go to
    /// the network if you are an authority
    /// for this hash.
    pub fn latest() -> Self {
        Self {
            strategy: GetStrategy::Latest,
        }
    }
    /// Gets the content but does not
    /// try to get the latest metadata.
    /// This will save a network call if the
    /// entry is local (cached, authored or integrated).
    ///
    /// This will fallback to the network if the content
    /// is not found locally
    pub fn content() -> Self {
        Self {
            strategy: GetStrategy::Content,
        }
    }
}

impl Default for GetOptions {
    fn default() -> Self {
        Self::latest()
    }
}

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
/// Describes the get call and what information
/// the caller is concerned about.
/// This helps the subconscious avoid unnecessary network calls.
pub enum GetStrategy {
    /// Will try to get the latest metadata but fallback
    /// to the cache if none is found.
    /// Does not go to the network if you are an authority for the data.
    Latest,
    /// Will try to get the content locally but go
    /// to the network if it is not found.
    /// Does not go to the network if you are an authority for the data.
    Content,
}

/// Zome input to create an entry.
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct CreateInput {
    /// The location this entry will be committed to.
    pub entry_location: EntryDefLocation,
    /// Entry body.
    pub entry: crate::entry::Entry,
    /// ChainTopBehaviour for the write.
    pub chain_top_ordering: ChainTopOrdering,
}

impl CreateInput {
    /// Constructor.
    pub fn new(
        entry_location: EntryDefLocation,
        entry: crate::entry::Entry,
        chain_top_ordering: ChainTopOrdering,
    ) -> Self {
        Self {
            entry_location,
            entry,
            chain_top_ordering,
        }
    }

    /// Consume into an Entry.
    pub fn into_entry(self) -> Entry {
        self.entry
    }

    /// Accessor.
    pub fn chain_top_ordering(&self) -> &ChainTopOrdering {
        &self.chain_top_ordering
    }
}

impl AsRef<crate::Entry> for CreateInput {
    fn as_ref(&self) -> &crate::Entry {
        &self.entry
    }
}

/// Zome input for get and get_details calls.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetInput {
    /// Any DHT hash to pass to get or get_details.
    pub any_dht_hash: holo_hash::AnyDhtHash,
    /// Options for the call.
    pub get_options: crate::entry::GetOptions,
}

impl GetInput {
    /// Constructor.
    pub fn new(any_dht_hash: holo_hash::AnyDhtHash, get_options: crate::entry::GetOptions) -> Self {
        Self {
            any_dht_hash,
            get_options,
        }
    }
}

/// Zome input type for all update operations.
#[derive(PartialEq, Debug, Deserialize, Serialize, Clone)]
pub struct UpdateInput {
    /// Header of the element being updated.
    pub original_header_address: holo_hash::HeaderHash,
    /// EntryDefId for the created entry
    pub entry_def_id: crate::entry_def::EntryDefId,
    /// Entry body.
    pub entry: crate::entry::Entry,
    /// ChainTopBehaviour for the write.
    pub chain_top_ordering: ChainTopOrdering,
}

/// Zome input for all delete operations.
#[derive(PartialEq, Debug, Deserialize, Serialize, Clone)]
pub struct DeleteInput {
    /// Header of the element being deleted.
    pub deletes_header_hash: holo_hash::HeaderHash,
    /// Chain top ordering behaviour for the delete.
    pub chain_top_ordering: ChainTopOrdering,
}

impl DeleteInput {
    /// Constructor.
    pub fn new(
        deletes_header_hash: holo_hash::HeaderHash,
        chain_top_ordering: ChainTopOrdering,
    ) -> Self {
        Self {
            deletes_header_hash,
            chain_top_ordering,
        }
    }
}

impl From<holo_hash::HeaderHash> for DeleteInput {
    /// Sets [`ChainTopOrdering`] to `default` = `Strict` when created from a hash.
    fn from(deletes_header_hash: holo_hash::HeaderHash) -> Self {
        Self {
            deletes_header_hash,
            chain_top_ordering: ChainTopOrdering::default(),
        }
    }
}

impl EntryDefLocation {
    /// Create an [`EntryDefLocation::App`].
    pub fn app(
        zome_name: impl Into<ZomeName>,
        app_entry_def_name: impl Into<AppEntryDefName>,
    ) -> Self {
        Self::App(AppEntryDefLocation {
            zome: zome_name.into(),
            entry: app_entry_def_name.into(),
        })
    }
}

impl From<(ZomeName, EntryDefId)> for EntryDefLocation {
    fn from((zome, e): (ZomeName, EntryDefId)) -> Self {
        match e {
            EntryDefId::App(entry) => Self::App(AppEntryDefLocation { zome, entry }),
            EntryDefId::CapClaim => Self::CapClaim,
            EntryDefId::CapGrant => Self::CapGrant,
        }
    }
}

impl From<(&str, EntryDefId)> for EntryDefLocation {
    fn from((z, e): (&str, EntryDefId)) -> Self {
        Self::from((ZomeName::from(z), e))
    }
}

impl From<(String, EntryDefId)> for EntryDefLocation {
    fn from((z, e): (String, EntryDefId)) -> Self {
        Self::from((ZomeName::from(z), e))
    }
}
