//! Defines the Chain Head Coordination API.
//!
//! **NOTE** this API is not set in stone. Do not design a CHC against this API yet,
//! as it will change!

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
};

use holo_hash::{ActionHash, EntryHash};
use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::prelude::*;

use crate::chain::{ChainItem, ChainItemHash};

/// The API which a Chain Head Coordinator service must implement.
///
/// **NOTE** this API is currently loosely defined and will certainly change
/// in the future. Do not write a real CHC according to this spec!
#[async_trait::async_trait]
pub trait ChainHeadCoordinator {
    /// The item which the chain is made of.
    type Item: ChainItem;

    /// Get just the head of the chain as recorded by the CHC.
    async fn head(&self) -> ChcResult<Option<ChainItemHash<Self::Item>>>;

    /// Add items to be appended to the CHC's chain.
    async fn add_actions(&self, actions: Vec<Self::Item>) -> ChcResult<()>;

    /// Add entries to the entry storage service.
    async fn add_entries(&self, entries: Vec<EntryHashed>) -> ChcResult<()>;

    /// Get actions including and beyond the given hash.
    async fn get_actions_since_hash(
        &self,
        hash: Option<ChainItemHash<Self::Item>>,
    ) -> ChcResult<Vec<Self::Item>>;

    /// Get entries with the given hashes. If any entries cannot be retrieved,
    /// an error will be returned. Otherwise, the hashmap will pair Entries with
    /// each hash requested.
    async fn get_entries(
        &self,
        hashes: HashSet<&EntryHash>,
    ) -> ChcResult<HashMap<EntryHash, Entry>>;
}

/// Assemble records from a list of Actions and a map of Entries
pub fn records_from_actions_and_entries(
    actions: Vec<SignedActionHashed>,
    mut entries: HashMap<EntryHash, Entry>,
) -> ChcResult<Vec<Record>> {
    let mut records = vec![];
    for action in actions {
        let entry = if let Some(hash) = action.hashed.entry_hash() {
            Some(
                entries
                    .remove(hash)
                    .ok_or_else(|| ChcError::MissingEntryForAction(action.as_hash().clone()))?,
            )
        } else {
            None
        };
        let record = Record::new(action, entry);
        records.push(record);
    }
    Ok(records)
}

#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum ChcError {
    #[error(transparent)]
    DeserializationError(#[from] SerializedBytesError),

    #[error("Adding these items to the CHC results in an invalid chain. Current CHC top sequence number: {0:?}, Error: {1}")]
    InvalidChain(Option<u32>, String),

    #[error("Missing Entry for ActionHash: {0:?}")]
    MissingEntryForAction(ActionHash),

    #[error("Missing Entry for EntryHash: {0:?}")]
    MissingEntries(HashSet<EntryHash>),

    #[error("The CHC service is unreachable: {0}")]
    ServiceUnreachable(String),
}

#[allow(missing_docs)]
pub type ChcResult<T> = Result<T, ChcError>;
