#![allow(missing_docs)]

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
};

use holo_hash::{ActionHash, EntryHash};
use holochain_serialized_bytes::SerializedBytesError;
use holochain_zome_types::prelude::*;

use crate::chain::{ChainItem, ChainItemHash};

#[async_trait::async_trait]
pub trait ChainHeadCoordinator {
    type Item: ChainItem;

    async fn head(&self) -> ChcResult<Option<ChainItemHash<Self::Item>>>;

    async fn add_actions(&self, actions: Vec<Self::Item>) -> ChcResult<()>;

    async fn add_entries(&self, entries: Vec<EntryHashed>) -> ChcResult<()>;

    async fn get_actions_since_hash(
        &self,
        hash: Option<ChainItemHash<Self::Item>>,
    ) -> ChcResult<Vec<Self::Item>>;

    async fn get_entries(
        &self,
        hashes: HashSet<&EntryHash>,
    ) -> ChcResult<HashMap<EntryHash, Entry>>;
}

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
        let record = Record::create(action, entry);
        records.push(record);
    }
    Ok(records)
}

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

pub type ChcResult<T> = Result<T, ChcError>;
