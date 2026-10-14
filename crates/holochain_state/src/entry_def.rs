use crate::prelude::StateMutationError;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use crate::query::StateQueryError;
use holo_hash::ZomeHash;
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

    /// Delete the entry definitions of an integrity zome that no stored DNA
    /// definition references any more.
    ///
    /// Does nothing if any stored DNA definition still has a zome with this
    /// hash, because the entry definitions are keyed by zome and shared by
    /// every DNA that includes it. Returns the number of entry definitions
    /// removed.
    pub async fn delete_for_unreferenced_zome(
        &self,
        zome_hash: &ZomeHash,
    ) -> StateMutationResult<u64> {
        if self.db.as_ref().zome_hash_referenced(zome_hash).await? {
            return Ok(0);
        }

        use holochain_serialized_bytes::SerializedBytes;
        let mut deleted = 0;
        for (key, _) in self.as_read().get_all().await? {
            if key.zome.zome_hash() != *zome_hash {
                continue;
            }
            let serialized: SerializedBytes = key.try_into()?;
            if self.db.delete_entry_def(serialized.bytes()).await? {
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    /// Downgrade this writable store to a read-only store.
    pub fn as_read(&self) -> EntryDefStoreRead {
        EntryDefStore::new(self.db.as_ref().clone())
    }

    /// Convert this writable store into a read-only store.
    pub fn into_read(self) -> EntryDefStoreRead {
        EntryDefStore::new(self.db.into())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dna_def::DnaDefStore;
    use holo_hash::{HashableContentExtAsync, HashableContentExtSync, WasmHash, ZomeHash};
    use holochain_types::prelude::{
        AgentPubKey, CoordinatorZome, CoordinatorZomeDef, DnaDef, DnaWasm, IntegrityZome,
        IntegrityZomeDef, ZomeName,
    };

    async fn wasm_hash(seed: u8) -> WasmHash {
        DnaWasm {
            code: vec![0x00, 0x61, 0x73, 0x6d, seed, 0, 0, 0].into(),
        }
        .to_hash()
        .await
    }

    fn entry_def(id: &str) -> EntryDef {
        EntryDef {
            id: EntryDefId::App(id.to_string().into()),
            visibility: EntryVisibility::Public,
            required_validations: 5u8.into(),
            cache_at_agent_activity: false,
        }
    }

    async fn test_db() -> holochain_data::DbWrite<holochain_data::kind::Wasm> {
        holochain_data::test_open_db(holochain_data::kind::Wasm)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn delete_for_unreferenced_zome_removes_only_that_zomes_defs() {
        let db = test_db().await;
        let store = EntryDefStore::new(db);

        let zome_a = IntegrityZomeDef::from_hash(wasm_hash(1).await);
        let zome_b = IntegrityZomeDef::from_hash(wasm_hash(2).await);

        store
            .put(
                EntryDefBufferKey::new(zome_a.clone(), EntryDefIndex(0)),
                &entry_def("a0"),
            )
            .await
            .unwrap();
        store
            .put(
                EntryDefBufferKey::new(zome_a.clone(), EntryDefIndex(1)),
                &entry_def("a1"),
            )
            .await
            .unwrap();
        store
            .put(
                EntryDefBufferKey::new(zome_b.clone(), EntryDefIndex(0)),
                &entry_def("b0"),
            )
            .await
            .unwrap();
        assert_eq!(store.as_read().get_all().await.unwrap().len(), 3);

        let deleted = store
            .delete_for_unreferenced_zome(&zome_a.zome_hash())
            .await
            .unwrap();
        assert_eq!(deleted, 2);

        let remaining = store.as_read().get_all().await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0.zome, zome_b);
        assert_eq!(remaining[0].1.id, EntryDefId::App("b0".into()));

        // A zome with no entry defs at all: nothing deleted, no error.
        let unknown: ZomeHash = wasm_hash(9).await.into();
        assert_eq!(
            store.delete_for_unreferenced_zome(&unknown).await.unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn delete_for_unreferenced_zome_keeps_defs_of_zome_still_in_use() {
        let db = test_db().await;
        let store = EntryDefStore::new(db.clone());
        let dna_defs = DnaDefStore::new(db);

        let integrity = wasm_hash(1).await;
        let zome = IntegrityZomeDef::from_hash(integrity.clone());
        store
            .put(
                EntryDefBufferKey::new(zome.clone(), EntryDefIndex(0)),
                &entry_def("a0"),
            )
            .await
            .unwrap();

        // A stored DNA definition references the zome.
        let dna_def = DnaDef::unique_from_zomes(
            vec![IntegrityZome::from((
                ZomeName::from("integrity"),
                zome.clone(),
            ))],
            vec![CoordinatorZome::from((
                ZomeName::from("coordinator"),
                CoordinatorZomeDef::from_hash(wasm_hash(2).await),
            ))],
        );
        let agent = AgentPubKey::from_raw_32(vec![1u8; 32]);
        dna_defs.put(&agent, &dna_def).await.unwrap();

        // Still referenced: nothing is deleted.
        let deleted = store
            .delete_for_unreferenced_zome(&zome.zome_hash())
            .await
            .unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(store.as_read().get_all().await.unwrap().len(), 1);

        // Once the DNA definition is gone, the entry defs go too.
        dna_defs.delete_for_dna(&dna_def.to_hash()).await.unwrap();
        let deleted = store
            .delete_for_unreferenced_zome(&zome.zome_hash())
            .await
            .unwrap();
        assert_eq!(deleted, 1);
        assert!(store.as_read().get_all().await.unwrap().is_empty());
    }
}
