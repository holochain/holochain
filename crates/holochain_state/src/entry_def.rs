use crate::prelude::StateMutationError;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use crate::query::StateQueryError;
use holochain_types::prelude::EntryDefBufferKey;
use holochain_zome_types::prelude::*;

/// A wrapper around the entry definition database for managing entry definition storage and retrieval.
#[derive(Clone)]
pub struct EntryDefStore<Db = holochain_data::DbWrite<holochain_data::kind::Wasm>> {
    db: Db,
}

/// A read-only view of the entry definition store.
pub type EntryDefStoreRead = EntryDefStore<holochain_data::DbRead<holochain_data::kind::Wasm>>;

impl<Db> EntryDefStore<Db> {
    /// Create a new EntryDefStore from a database handle.
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl EntryDefStore<holochain_data::DbRead<holochain_data::kind::Wasm>> {
    /// Retrieve all entry definitions from the database.
    pub async fn get_all(&self) -> StateQueryResult<Vec<(EntryDefBufferKey, EntryDef)>> {
        let all_entry_defs = self
            .db
            .get_all_entry_defs()
            .await
            .map_err(StateQueryError::from)?;

        use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
        all_entry_defs
            .into_iter()
            .map(|(key_bytes, entry_def)| {
                let serialized = SerializedBytes::from(UnsafeBytes::from(key_bytes));
                let key: EntryDefBufferKey = serialized.try_into().map_err(
                    |e: holochain_serialized_bytes::SerializedBytesError| StateQueryError::from(e),
                )?;
                Ok((key, entry_def))
            })
            .collect()
    }
}

impl EntryDefStore<holochain_data::DbWrite<holochain_data::kind::Wasm>> {
    /// Store an entry definition in the database.
    pub async fn put(
        &self,
        key: EntryDefBufferKey,
        entry_def: &EntryDef,
    ) -> StateMutationResult<()> {
        use holochain_serialized_bytes::SerializedBytes;
        let serialized: SerializedBytes =
            key.try_into()
                .map_err(|e: holochain_serialized_bytes::SerializedBytesError| {
                    StateMutationError::from(e)
                })?;
        let key_bytes = serialized.bytes().to_vec();
        self.db
            .put_entry_def(key_bytes, entry_def)
            .await
            .map_err(StateMutationError::from)
    }

    /// Downgrade this writable store to a read-only store.
    pub fn as_read(&self) -> EntryDefStoreRead {
        EntryDefStore::new(self.db.as_ref().clone())
    }

    /// Convert this writable store into a read-only store.
    pub fn into_read(self) -> EntryDefStoreRead {
        EntryDefStore::new(self.db.as_ref().clone())
    }
}

impl From<EntryDefStore<holochain_data::DbWrite<holochain_data::kind::Wasm>>>
    for EntryDefStoreRead
{
    fn from(store: EntryDefStore<holochain_data::DbWrite<holochain_data::kind::Wasm>>) -> Self {
        store.into_read()
    }
}

#[cfg(feature = "test_utils")]
impl<Db> EntryDefStore<Db>
where
    Db: AsRef<holochain_data::DbRead<holochain_data::kind::Wasm>>,
{
    /// Get a reference to the raw database handle for testing purposes.
    pub fn raw_db_read(&self) -> &holochain_data::DbRead<holochain_data::kind::Wasm> {
        self.db.as_ref()
    }
}
