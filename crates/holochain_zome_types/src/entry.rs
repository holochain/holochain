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

// Re-export GetStrategy from holochain_integrity_types for backward compatibility
pub use holochain_integrity_types::get_strategy::GetStrategy;

/// Maximum number of remote agents that can be queried in parallel.
/// This limit prevents abuse and excessive network load.
pub const MAX_REMOTE_AGENT_COUNT: u8 = 5;

mod app_entry_bytes;
pub use app_entry_bytes::*;
pub use holochain_integrity_types::entry::*;

/// Either an [`EntryDefIndex`] or one of:
/// - [EntryType::CapGrant]
/// - [EntryType::CapClaim]
///
/// Which don't have an index.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum EntryDefLocation {
    /// App defined entries always have a unique [`u8`] index
    /// within the Dna.
    App(AppEntryDefLocation),
    /// [`CapClaim`](holochain_integrity_types::EntryDefId::CapClaim) is committed to and
    /// validated by all integrity zomes in the dna.
    CapClaim,
    /// [`CapGrant`](holochain_integrity_types::EntryDefId::CapGrant) is committed to and
    /// validated by all integrity zomes in the dna.
    CapGrant,
}

/// The location of an app entry definition.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct AppEntryDefLocation {
    /// The zome that defines this entry type.
    pub zome_index: ZomeIndex,
    /// The entry type within the zome.
    pub entry_def_index: EntryDefIndex,
}

/// Options for controlling how get is executed.
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct GetOptions {
    /// Configure whether data should be fetched from the network or only from the local databases.
    strategy: GetStrategy,

    /// Number of remote agents to query in parallel.
    ///
    /// Only used when strategy is [`GetStrategy::Network`].
    ///
    /// `None` means use the conductor's default.
    ///
    /// A maximum of [`MAX_REMOTE_AGENT_COUNT`] is enforced for this value.
    remote_agent_count: Option<u8>,

    /// Timeout for network requests in milliseconds.
    ///
    /// Only used when strategy is [`GetStrategy::Network`].
    ///
    /// None means use conductor settings.
    timeout_ms: Option<u64>,

    /// Whether race mode is enabled.
    ///
    /// Performing a race means that multiple requests are made in parallel and the first result is used.
    ///
    /// Only used when strategy is [`GetStrategy::Network`] and the remote agent count is >= 2.
    ///
    /// Note: Setting this to false is not yet implemented.
    ///
    /// None means use the network's default (race).
    as_race: Option<bool>,
}

impl GetOptions {
    /// Get the strategy for this request.
    pub fn strategy(&self) -> GetStrategy {
        self.strategy
    }

    /// Get the number of remote agents to query.
    pub fn remote_agent_count(&self) -> Option<u8> {
        self.remote_agent_count
    }

    /// Get the timeout in milliseconds.
    pub fn timeout_ms(&self) -> Option<u64> {
        self.timeout_ms
    }

    /// Get whether to race or aggregate responses.
    pub fn as_race(&self) -> Option<bool> {
        self.as_race
    }

    /// Set the strategy while preserving other options.
    pub fn with_strategy(mut self, strategy: GetStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Fetch latest metadata from the network,
    /// and otherwise fall back to locally cached metadata.
    ///
    /// If the current agent is an authority for this hash, this call will not
    /// go to the network.
    pub fn network() -> Self {
        Self {
            strategy: GetStrategy::Network,
            remote_agent_count: None,
            timeout_ms: None,
            as_race: None,
        }
    }

    /// Gets the action/entry and its metadata from local databases only.
    /// No network call is made.
    pub fn local() -> Self {
        Self {
            strategy: GetStrategy::Local,
            remote_agent_count: None,
            timeout_ms: None,
            as_race: None,
        }
    }

    /// Set the number of remote agents to query.
    ///
    /// The count will be capped at [`MAX_REMOTE_AGENT_COUNT`] to prevent abuse
    /// and excessive network load.
    pub fn with_remote_agent_count(mut self, count: u8) -> Self {
        self.remote_agent_count = Some(count.min(MAX_REMOTE_AGENT_COUNT));
        self
    }

    /// Set the timeout for network requests in milliseconds.
    pub fn with_timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = Some(timeout);
        self
    }

    /// Set whether to race (true) or aggregate (false) responses.
    /// Note: Setting as_race to false is not yet implemented.
    pub fn with_as_race(mut self, race: bool) -> Self {
        self.as_race = Some(race);
        self
    }
}

impl Default for GetOptions {
    fn default() -> Self {
        Self::network()
    }
}

impl From<GetStrategy> for GetOptions {
    fn from(strategy: GetStrategy) -> Self {
        Self {
            strategy,
            remote_agent_count: None,
            timeout_ms: None,
            as_race: None,
        }
    }
}

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

impl UpdateInput {
    /// Constructor.
    pub fn new(
        original_action_address: holo_hash::ActionHash,
        entry: crate::entry::Entry,
        chain_top_ordering: ChainTopOrdering,
    ) -> Self {
        Self {
            original_action_address,
            entry,
            chain_top_ordering,
        }
    }
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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_should_convert_get_strategy_to_get_options() {
        let get_options = GetOptions::from(GetStrategy::Network);
        assert_eq!(get_options.strategy(), GetStrategy::Network);

        let get_options = GetOptions::from(GetStrategy::Local);
        assert_eq!(get_options.strategy(), GetStrategy::Local);
    }

    #[test]
    fn test_get_options_builder() {
        let options = GetOptions::network()
            .with_remote_agent_count(5)
            .with_timeout_ms(2000)
            .with_as_race(true);

        assert_eq!(options.strategy(), GetStrategy::Network);
        assert_eq!(options.remote_agent_count(), Some(5));
        assert_eq!(options.timeout_ms(), Some(2000));
        assert_eq!(options.as_race(), Some(true));

        // Test capping of remote_agent_count
        let options = GetOptions::network().with_remote_agent_count(20);
        assert_eq!(options.remote_agent_count(), Some(5)); // Capped at MAX_REMOTE_AGENT_COUNT

        // Test that values at or below the max are preserved
        let options = GetOptions::network().with_remote_agent_count(5);
        assert_eq!(options.remote_agent_count(), Some(5));

        let options = GetOptions::network().with_remote_agent_count(3);
        assert_eq!(options.remote_agent_count(), Some(3));
    }
}
