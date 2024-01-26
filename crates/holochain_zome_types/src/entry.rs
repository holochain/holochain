//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::action::ChainTopOrdering;
use holochain_integrity_types::EntryDefIndex;
use holochain_integrity_types::EntryType;
use holochain_integrity_types::EntryVisibility;
use holochain_integrity_types::ScopedEntryDefIndex;
use holochain_integrity_types::ZomeIndex;
use holochain_serialized_bytes::prelude::*;

mod app_entry_bytes;
pub use app_entry_bytes::*;

pub use holochain_integrity_types::entry::*;

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
/// Either an [`EntryDefIndex`] or one of:
/// - [`EntryType::CapGrant`](crate::prelude::EntryType::CapGrant)
/// - [`EntryType::CapClaim`](crate::prelude::EntryType::CapClaim)
/// Which don't have an index.
pub enum EntryDefLocation {
    /// App defined entries always have a unique [`u8`] index
    /// within the Dna.
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
/// The location of an app entry definition.
pub struct AppEntryDefLocation {
    /// The zome that defines this entry type.
    pub zome_index: ZomeIndex,
    /// The entry type within the zome.
    pub entry_def_index: EntryDefIndex,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
/// Options for controlling how get works
pub struct GetOptions {
    /// Controls whether data will be fetched only
    pub style: GetStyle,

    /// If this is true the get call will wait for
    /// the latest data before returning.
    /// If it is false you will get whatever is locally
    /// available on this conductor.
    #[deprecated = "Use GetOptions::style instead"]
    pub strategy: GetStrategy,
}

#[allow(deprecated)]
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
            style: GetStyle::NetworkBlocking,
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
            style: GetStyle::NetworkBlocking,
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

/// Describes distinct methods of performing a get.
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum GetStyle {
    /// Always go to the network to look for data and block until a response
    /// is received or the call times out.
    ///
    /// If you already have a definitive response to the query stored locally,
    /// a network call may not be necessary.
    /// For instance, if you're getting a single entry and you already have the
    /// entry stored locally, there's no need to go to the network.
    /// However, if you're looking for links or potential updates and deletes to
    /// content, this will always go to the network to look for the latest data.
    NetworkBlocking,

    /// Returns immediately with whatever local data is available, while also
    /// triggering an async network request. If and when the network request
    /// returns, a signal will be sent over the App websocket with the results,
    /// including the provided subscription token.
    NetworkSubscription(NetworkSubscriptionToken),

    /// Never go to the network, and only query local storage.
    /// The get returns immediately, but the data may be out of date.
    /// Note that every network request is cached, so subsequent LocalOnly calls
    /// will include data returned from recent network calls.
    LocalOnly,
}

impl Default for GetStyle {
    fn default() -> Self {
        Self::NetworkBlocking
    }
}

/// When using [`GetStyle::NetworkSubscription`], the caller must provide a
/// subscription token. This token is used to correlate the response with the
/// original request when handling the signal.
pub type NetworkSubscriptionToken = String;

/// Zome input to create an entry.
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct CreateInput {
    /// The global type index for this entry (if it has one).
    pub entry_location: EntryDefLocation,
    /// The visibility of this entry.
    pub entry_visibility: EntryVisibility,
    /// Entry body.
    pub entry: crate::entry::Entry,
    /// ChainTopBehaviour for the write.
    pub chain_top_ordering: ChainTopOrdering,
}

impl CreateInput {
    /// Constructor.
    pub fn new(
        entry_location: impl Into<EntryDefLocation>,
        entry_visibility: EntryVisibility,
        entry: crate::entry::Entry,
        chain_top_ordering: ChainTopOrdering,
    ) -> Self {
        Self {
            entry_location: entry_location.into(),
            entry_visibility,
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
    /// Action of the record being updated.
    pub original_action_address: holo_hash::ActionHash,
    /// Entry body.
    pub entry: crate::entry::Entry,
    /// ChainTopBehaviour for the write.
    pub chain_top_ordering: ChainTopOrdering,
}

/// Zome input for all delete operations.
#[derive(PartialEq, Debug, Deserialize, Serialize, Clone)]
pub struct DeleteInput {
    /// Action of the record being deleted.
    pub deletes_action_hash: holo_hash::ActionHash,
    /// Chain top ordering behaviour for the delete.
    pub chain_top_ordering: ChainTopOrdering,
}

impl DeleteInput {
    /// Constructor.
    pub fn new(
        deletes_action_hash: holo_hash::ActionHash,
        chain_top_ordering: ChainTopOrdering,
    ) -> Self {
        Self {
            deletes_action_hash,
            chain_top_ordering,
        }
    }
}

impl From<holo_hash::ActionHash> for DeleteInput {
    /// Sets [`ChainTopOrdering`] to `default` = `Strict` when created from a hash.
    fn from(deletes_action_hash: holo_hash::ActionHash) -> Self {
        Self {
            deletes_action_hash,
            chain_top_ordering: ChainTopOrdering::default(),
        }
    }
}

impl EntryDefLocation {
    /// Create an [`EntryDefLocation::App`].
    pub fn app(
        zome_index: impl Into<ZomeIndex>,
        entry_def_index: impl Into<EntryDefIndex>,
    ) -> Self {
        Self::App(AppEntryDefLocation {
            zome_index: zome_index.into(),
            entry_def_index: entry_def_index.into(),
        })
    }
}

impl From<ScopedEntryDefIndex> for AppEntryDefLocation {
    fn from(s: ScopedEntryDefIndex) -> Self {
        Self {
            zome_index: s.zome_index,
            entry_def_index: s.zome_type,
        }
    }
}

impl From<ScopedEntryDefIndex> for EntryDefLocation {
    fn from(s: ScopedEntryDefIndex) -> Self {
        Self::App(s.into())
    }
}

/// Check the entry variant matches the variant in the actions entry type
pub fn entry_type_matches(entry_type: &EntryType, entry: &Entry) -> bool {
    #[allow(clippy::match_like_matches_macro)]
    match (entry_type, entry) {
        (EntryType::AgentPubKey, Entry::Agent(_)) => true,
        (EntryType::App(_), Entry::App(_)) => true,
        (EntryType::App(_), Entry::CounterSign(_, _)) => true,
        (EntryType::CapClaim, Entry::CapClaim(_)) => true,
        (EntryType::CapGrant, Entry::CapGrant(_)) => true,
        _ => false,
    }
}
