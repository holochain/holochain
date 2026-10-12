use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_types::prelude::CellId;
use holochain_types::prelude::DnaDef;
use holochain_zome_types::prelude::DnaDefHashed;
use std::collections::HashSet;

/// A wrapper around the DNA definition database for managing DNA definition storage and retrieval.
#[derive(Clone)]
pub struct DnaDefStore<Db = holochain_data::DbWrite<holochain_data::kind::Wasm>> {
    db: Db,
}

/// A read-only view of the DNA definition store.
pub type DnaDefStoreRead = DnaDefStore<holochain_data::DbRead<holochain_data::kind::Wasm>>;

impl<Db> DnaDefStore<Db> {
    /// Create a new DnaDefStore from a database handle.
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl DnaDefStore<holochain_data::DbRead<holochain_data::kind::Wasm>> {
    /// Retrieve a DNA definition from the database by its cell ID.
    pub async fn get(&self, cell_id: &CellId) -> StateQueryResult<Option<DnaDefHashed>> {
        Ok(self.db.get_dna_def(cell_id).await?)
    }

    /// Retrieve every DNA definition in the store with its cell ID.
    pub async fn get_all(&self) -> StateQueryResult<Vec<(CellId, DnaDefHashed)>> {
        Ok(self.db.get_all_dna_defs().await?)
    }

    /// List the distinct DNA hashes of all DNA definitions in the database.
    pub async fn list_dna_hashes(&self) -> StateQueryResult<HashSet<DnaHash>> {
        Ok(self
            .db
            .get_all_dna_defs()
            .await?
            .into_iter()
            .map(|(cell_id, _)| cell_id.dna_hash().clone())
            .collect())
    }
}

impl DnaDefStore<holochain_data::DbWrite<holochain_data::kind::Wasm>> {
    /// Store or update a DNA definition in the database.
    pub async fn put(&self, agent: &AgentPubKey, dna_def: &DnaDef) -> StateMutationResult<()> {
        self.db.put_dna_def(agent, dna_def).await?;
        Ok(())
    }

    /// Delete every stored definition of this DNA, for every agent.
    ///
    /// Returns the number of definitions removed. WASM code and entry
    /// definitions are not touched; see
    /// [`WasmStore::delete_if_unreferenced`](crate::wasm::WasmStore::delete_if_unreferenced)
    /// and
    /// [`EntryDefStore::delete_for_unreferenced_zome`](crate::entry_def::EntryDefStore::delete_for_unreferenced_zome).
    pub async fn delete_for_dna(&self, dna_hash: &DnaHash) -> StateMutationResult<u64> {
        Ok(self.db.delete_dna_defs(dna_hash).await?)
    }

    /// Atomically delete a DNA's definitions and resources no other DNA references.
    ///
    /// Reads every agent-specific definition and removes unused entry definitions,
    /// WASM, and compiled WASM in one transaction. Shared resources are retained.
    /// A failure or cancellation before commit rolls back all deletions.
    ///
    /// # Errors
    /// Returns an error if resource queries, key decoding, deletions, or commit fail.
    pub async fn delete_with_resources(&self, dna_hash: &DnaHash) -> StateMutationResult<()> {
        use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
        use holochain_types::prelude::{EntryDefBufferKey, WasmZomeDef, ZomeDef};

        let mut tx = self.db.begin().await?;
        // Coordinator zomes can differ across agents without changing the DNA hash.
        let dna_defs = tx
            .as_mut()
            .get_all_dna_defs()
            .await?
            .into_iter()
            .filter(|(cell_id, _)| cell_id.dna_hash() == dna_hash)
            .map(|(_, dna_def)| dna_def)
            .collect::<Vec<_>>();
        tx.delete_dna_defs(dna_hash).await?;

        let mut integrity_hashes = HashSet::new();
        let mut wasm_hashes = HashSet::new();
        for dna_def in &dna_defs {
            integrity_hashes.extend(
                dna_def
                    .integrity_zomes
                    .iter()
                    .map(|(_, zome)| zome.zome_hash()),
            );
            for (_, zome) in dna_def.all_zomes() {
                if let ZomeDef::Wasm(WasmZomeDef { wasm_hash, .. }) = zome {
                    wasm_hashes.insert(wasm_hash.clone());
                }
            }
        }
        let mut unreferenced = HashSet::new();
        for hash in integrity_hashes {
            if !tx.as_mut().zome_hash_referenced(&hash).await? {
                unreferenced.insert(hash);
            }
        }
        if !unreferenced.is_empty() {
            for (key_bytes, _) in tx.as_mut().get_all_entry_defs().await? {
                let key: EntryDefBufferKey =
                    SerializedBytes::from(UnsafeBytes::from(key_bytes.clone())).try_into()?;
                if unreferenced.contains(&key.zome.zome_hash()) {
                    tx.delete_entry_def(&key_bytes).await?;
                }
            }
        }
        for hash in wasm_hashes {
            tx.delete_wasm_if_unreferenced(&hash).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Downgrade this writable store to a read-only store.
    pub fn as_read(&self) -> DnaDefStoreRead {
        DnaDefStore::new(self.db.as_ref().clone())
    }

    /// Convert this writable store into a read-only store.
    pub fn into_read(self) -> DnaDefStoreRead {
        DnaDefStore::new(self.db.into())
    }
}

impl From<DnaDefStore<holochain_data::DbWrite<holochain_data::kind::Wasm>>> for DnaDefStoreRead {
    fn from(store: DnaDefStore<holochain_data::DbWrite<holochain_data::kind::Wasm>>) -> Self {
        store.into_read()
    }
}

#[cfg(feature = "test_utils")]
impl<Db> DnaDefStore<Db>
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
    use holochain_types::prelude::*;

    #[tokio::test]
    async fn failed_resource_cleanup_keeps_dna_definitions() {
        let db = holochain_data::test_open_db(holochain_data::kind::Wasm)
            .await
            .unwrap();
        let store = DnaDefStore::new(db.clone());
        let wasm = DnaWasmHashed::from_content(DnaWasm {
            code: vec![1, 2, 3].into(),
        })
        .await;
        db.put_wasm(wasm.clone()).await.unwrap();
        db.put_compiled_wasm(
            wasm.as_hash().clone(),
            bytes::Bytes::from_static(b"compiled"),
        )
        .await
        .unwrap();
        let zome = IntegrityZomeDef::from_hash(wasm.as_hash().clone());
        let dna = DnaDef::unique_from_zomes(
            vec![IntegrityZome::from((
                ZomeName::from("integrity"),
                zome.clone(),
            ))],
            Vec::new(),
        );
        let agent = AgentPubKey::from_raw_32(vec![1; 32]);
        store.put(&agent, &dna).await.unwrap();
        let entries = crate::entry_def::EntryDefStore::new(db.clone());
        entries
            .put(
                EntryDefBufferKey::new(zome, EntryDefIndex(0)),
                &EntryDef {
                    id: EntryDefId::App("entry".into()),
                    visibility: EntryVisibility::Public,
                    required_validations: 5.into(),
                    cache_at_agent_activity: false,
                },
            )
            .await
            .unwrap();
        sqlx::query(
            "CREATE TRIGGER reject_entry_cleanup BEFORE DELETE ON EntryDef
                     BEGIN SELECT RAISE(ABORT, 'cleanup interrupted'); END",
        )
        .execute(db.pool())
        .await
        .unwrap();

        assert!(store.delete_with_resources(&dna.to_hash()).await.is_err());
        assert!(store
            .as_read()
            .get(&CellId::new(dna.to_hash(), agent))
            .await
            .unwrap()
            .is_some());
        assert_eq!(entries.as_read().get_all().await.unwrap().len(), 1);
        assert!(db.as_ref().wasm_exists(wasm.as_hash()).await.unwrap());
        assert!(db
            .as_ref()
            .compiled_wasm_exists(wasm.as_hash())
            .await
            .unwrap());

        sqlx::query("DROP TRIGGER reject_entry_cleanup")
            .execute(db.pool())
            .await
            .unwrap();
        store.delete_with_resources(&dna.to_hash()).await.unwrap();
        assert!(store.as_read().get_all().await.unwrap().is_empty());
        assert!(entries.as_read().get_all().await.unwrap().is_empty());
        assert!(!db.as_ref().wasm_exists(wasm.as_hash()).await.unwrap());
        assert!(!db
            .as_ref()
            .compiled_wasm_exists(wasm.as_hash())
            .await
            .unwrap());
    }
}
