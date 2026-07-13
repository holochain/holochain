pub use error::*;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::AnyDhtHashPrimitive;
use holo_hash::EntryHash;
use holochain_types::prelude::*;

pub mod error;
pub mod link;

pub mod prelude {
    pub use super::StateQueryResult;
    pub use super::Store;
}

pub trait Store {
    /// Get an [`Entry`] from this store.
    fn get_entry(&self, hash: &EntryHash) -> StateQueryResult<Option<Entry>>;

    /// Get an [`Entry`] from this store.
    /// - Will return any public entry.
    /// - If an author is provided and an action for this entry matches the author then any entry
    ///   will be return regardless of visibility.
    fn get_public_or_authored_entry(
        &self,
        hash: &EntryHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Entry>>;

    /// Get an [`SignedActionHashed`] from this store.
    fn get_action(&self, hash: &ActionHash) -> StateQueryResult<Option<SignedActionHashed>>;

    /// Get a [`Warrant`] from this store.
    /// The second parameter determines whether the warrant op should be checked for validity.
    /// It should be set to false if reading from an Authored DB, where everything is valid,
    /// and true if reading from a DHT DB, where validation status matters
    fn get_warrants_for_agent(
        &self,
        agent_key: &AgentPubKey,
        check_valid: bool,
    ) -> StateQueryResult<Vec<WarrantOp>>;

    /// Get a [`Record`] from this store which includes the [`Entry`] if present.
    fn get_record(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<Record>>;

    /// Get a [`Record`] from this store. If an [`Entry`] is associated with the [`Action`],
    /// it will be included. But should the entry not be available, no record is returned.
    fn get_public_record(&self, hash: &AnyDhtHash) -> StateQueryResult<Option<Record>>;

    /// Get an [`Record`] from this store that is either public or
    /// authored by the given key.
    fn get_public_or_authored_record(
        &self,
        hash: &AnyDhtHash,
        author: Option<&AgentPubKey>,
    ) -> StateQueryResult<Option<Record>>;

    /// Check if a hash is contained in the store
    fn contains_hash(&self, hash: &AnyDhtHash) -> StateQueryResult<bool> {
        match hash.clone().into_primitive() {
            AnyDhtHashPrimitive::Entry(hash) => self.contains_entry(&hash),
            AnyDhtHashPrimitive::Action(hash) => self.contains_action(&hash),
        }
    }

    /// Check if an entry is contained in the store
    fn contains_entry(&self, hash: &EntryHash) -> StateQueryResult<bool>;

    /// Check if an action is contained in the store
    fn contains_action(&self, hash: &ActionHash) -> StateQueryResult<bool>;
}
