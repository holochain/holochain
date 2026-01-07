//! Operations for the Wasm database.
//!
//! The Wasm database stores DNA definitions, WASM bytecode, and entry definitions.

use crate::handles::{DbRead, DbWrite};
use crate::kind::Wasm;
use crate::models::{
    CoordinatorZomeModel, DnaDefModel, EntryDefModel, IntegrityZomeModel, WasmModel,
};
use holo_hash::HashableContentExtSync;
use holo_hash::{DnaHash, WasmHash};
use holochain_integrity_types::prelude::EntryDef;
use holochain_types::prelude::{DnaDef, DnaWasmHashed};
use holochain_zome_types::zome::{WasmZome, ZomeDef};

// Read operations
impl DbRead<Wasm> {
    /// Check if WASM bytecode exists in the database.
    pub async fn wasm_exists(&self, hash: &WasmHash) -> sqlx::Result<bool> {
        let hash_bytes = hash.get_raw_39();
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM Wasm WHERE hash = ?)")
            .bind(hash_bytes)
            .fetch_one(self.pool())
            .await?;
        Ok(exists)
    }

    /// Get WASM bytecode by hash.
    pub async fn get_wasm(&self, hash: &WasmHash) -> sqlx::Result<Option<DnaWasmHashed>> {
        let hash_bytes = hash.get_raw_39();
        let model: Option<WasmModel> = sqlx::query_as("SELECT hash, code FROM Wasm WHERE hash = ?")
            .bind(hash_bytes)
            .fetch_optional(self.pool())
            .await?;

        match model {
            Some(model) => {
                let wasm_hash = model.wasm_hash();
                Ok(Some(DnaWasmHashed::with_pre_hashed(
                    model.code.into(),
                    wasm_hash,
                )))
            }
            None => Ok(None),
        }
    }

    /// Check if a DNA definition exists in the database.
    pub async fn dna_def_exists(&self, hash: &DnaHash) -> sqlx::Result<bool> {
        let hash_bytes = hash.get_raw_39();
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM DnaDef WHERE hash = ?)")
            .bind(hash_bytes)
            .fetch_one(self.pool())
            .await?;
        Ok(exists)
    }

    /// Get a DNA definition by hash.
    pub async fn get_dna_def(&self, hash: &DnaHash) -> sqlx::Result<Option<DnaDef>> {
        let hash_bytes = hash.get_raw_39();

        // Fetch the DnaDef model
        let dna_model: Option<DnaDefModel> = sqlx::query_as(
            "SELECT hash, name, network_seed, properties, lineage FROM DnaDef WHERE hash = ?",
        )
        .bind(hash_bytes)
        .fetch_optional(self.pool())
        .await?;

        let dna_model = match dna_model {
            Some(m) => m,
            None => return Ok(None),
        };

        // Fetch integrity zomes
        let integrity_zomes: Vec<IntegrityZomeModel> = sqlx::query_as(
            "SELECT dna_hash, zome_index, zome_name, wasm_hash, dependencies FROM IntegrityZome WHERE dna_hash = ? ORDER BY zome_index",
        )
        .bind(hash_bytes)
        .fetch_all(self.pool())
        .await?;

        // Fetch coordinator zomes
        let coordinator_zomes: Vec<CoordinatorZomeModel> = sqlx::query_as(
            "SELECT dna_hash, zome_index, zome_name, wasm_hash, dependencies FROM CoordinatorZome WHERE dna_hash = ? ORDER BY zome_index",
        )
        .bind(hash_bytes)
        .fetch_all(self.pool())
        .await?;

        // Convert to DnaDef
        dna_model
            .to_dna_def(integrity_zomes, coordinator_zomes)
            .map(Some)
            .map_err(|e| {
                sqlx::Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            })
    }

    /// Check if an entry definition exists in the database.
    pub async fn entry_def_exists(&self, key: &[u8]) -> sqlx::Result<bool> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM EntryDef WHERE key = ?)")
                .bind(key)
                .fetch_one(self.pool())
                .await?;
        Ok(exists)
    }

    /// Get an entry definition by key.
    pub async fn get_entry_def(&self, _key: &[u8]) -> sqlx::Result<Option<EntryDef>> {
        let model: Option<EntryDefModel> = sqlx::query_as(
            "SELECT key, entry_def_id, entry_def_id_type, visibility, required_validations FROM EntryDef WHERE key = ?",
        )
        .bind(_key)
        .fetch_optional(self.pool())
        .await?;

        match model {
            Some(model) => model.to_entry_def().map(Some).map_err(|e| {
                sqlx::Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            }),
            None => Ok(None),
        }
    }

    /// Get all entry definitions.
    pub async fn get_all_entry_defs(&self) -> sqlx::Result<Vec<(Vec<u8>, EntryDef)>> {
        let models: Vec<EntryDefModel> = sqlx::query_as(
            "SELECT key, entry_def_id, entry_def_id_type, visibility, required_validations FROM EntryDef",
        )
        .fetch_all(self.pool())
        .await?;

        models
            .into_iter()
            .map(|model| {
                let key = model.key.clone();
                model
                    .to_entry_def()
                    .map(|entry_def| (key, entry_def))
                    .map_err(|e| {
                        sqlx::Error::Decode(Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            e,
                        )))
                    })
            })
            .collect()
    }
}

// Write operations
impl DbWrite<Wasm> {
    /// Store WASM bytecode.
    pub async fn put_wasm(&self, wasm: DnaWasmHashed) -> sqlx::Result<()> {
        let (wasm, hash) = wasm.into_inner();
        let hash_bytes = hash.get_raw_39();
        let code = wasm.code.to_vec();

        sqlx::query("INSERT OR REPLACE INTO Wasm (hash, code) VALUES (?, ?)")
            .bind(hash_bytes)
            .bind(code)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Store a DNA definition and its associated zomes.
    ///
    /// This operation is transactional - either all data is stored or none is.
    pub async fn put_dna_def(&self, dna_def: &DnaDef) -> sqlx::Result<()> {
        let mut tx = self.pool().begin().await?;

        let hash = dna_def.to_hash();
        let hash_bytes = hash.get_raw_39();
        let name = dna_def.name.clone();
        let network_seed = dna_def.modifiers.network_seed.clone();
        let properties = dna_def.modifiers.properties.bytes().to_vec();

        // Serialize lineage if present
        #[cfg(feature = "unstable-migration")]
        let lineage_json = Some(sqlx::types::Json(&dna_def.lineage));
        #[cfg(not(feature = "unstable-migration"))]
        let lineage_json: Option<
            sqlx::types::Json<&std::collections::HashSet<holochain_types::dna::DnaHash>>,
        > = None;

        // Insert DnaDef
        sqlx::query(
            "INSERT OR REPLACE INTO DnaDef (hash, name, network_seed, properties, lineage) VALUES (?, ?, ?, ?, ?)",
        )
            .bind(hash_bytes)
            .bind(name)
            .bind(network_seed)
            .bind(properties)
            .bind(lineage_json)
            .execute(&mut *tx)
            .await?;

        // Delete existing zomes for this DNA to avoid orphans when updating
        sqlx::query("DELETE FROM IntegrityZome WHERE dna_hash = ?")
            .bind(hash_bytes)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM CoordinatorZome WHERE dna_hash = ?")
            .bind(hash_bytes)
            .execute(&mut *tx)
            .await?;

        // Insert integrity zomes
        for (zome_index, (zome_name, zome_def)) in dna_def.integrity_zomes.iter().enumerate() {
            let wasm_hash = zome_def.wasm_hash(zome_name).map_err(|e| {
                sqlx::Error::Encode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                )))
            })?;
            let wasm_hash_bytes = wasm_hash.get_raw_39();

            // Extract dependencies from the ZomeDef
            let dependencies = match zome_def.as_any_zome_def() {
                ZomeDef::Wasm(WasmZome { dependencies, .. }) => dependencies
                    .iter()
                    .map(|n| n.0.as_ref().to_string())
                    .collect::<Vec<_>>(),
                ZomeDef::Inline { dependencies, .. } => dependencies
                    .iter()
                    .map(|n| n.0.as_ref().to_string())
                    .collect::<Vec<_>>(),
            };

            sqlx::query(
                "INSERT OR REPLACE INTO IntegrityZome (dna_hash, zome_index, zome_name, wasm_hash, dependencies) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(hash_bytes)
            .bind(zome_index as i64)
            .bind(&zome_name.0)
            .bind(wasm_hash_bytes)
            .bind(sqlx::types::Json(&dependencies))
            .execute(&mut *tx)
            .await?;
        }

        // Insert coordinator zomes
        for (zome_index, (zome_name, zome_def)) in dna_def.coordinator_zomes.iter().enumerate() {
            let wasm_hash = zome_def.wasm_hash(zome_name).map_err(|e| {
                sqlx::Error::Encode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                )))
            })?;
            let wasm_hash_bytes = wasm_hash.get_raw_39();

            // Extract dependencies from the ZomeDef
            let dependencies = match zome_def.as_any_zome_def() {
                ZomeDef::Wasm(WasmZome { dependencies, .. }) => dependencies
                    .iter()
                    .map(|n| n.0.as_ref().to_string())
                    .collect::<Vec<_>>(),
                ZomeDef::Inline { dependencies, .. } => dependencies
                    .iter()
                    .map(|n| n.0.as_ref().to_string())
                    .collect::<Vec<_>>(),
            };

            sqlx::query(
                "INSERT OR REPLACE INTO CoordinatorZome (dna_hash, zome_index, zome_name, wasm_hash, dependencies) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(hash_bytes)
            .bind(zome_index as i64)
            .bind(&zome_name.0)
            .bind(wasm_hash_bytes)
            .bind(sqlx::types::Json(&dependencies))
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Store an entry definition.
    pub async fn put_entry_def(&self, _key: Vec<u8>, _entry_def: &EntryDef) -> sqlx::Result<()> {
        let model = EntryDefModel::from_entry_def(_key, _entry_def);
        sqlx::query(
            "INSERT OR REPLACE INTO EntryDef (key, entry_def_id, entry_def_id_type, visibility, required_validations) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(model.key)
        .bind(model.entry_def_id)
        .bind(model.entry_def_id_type)
        .bind(model.visibility)
        .bind(model.required_validations)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::Wasm;
    use crate::test_setup_holochain_data;
    use holo_hash::HasHash;
    use holo_hash::HashableContentExtAsync;
    use holochain_integrity_types::{zome::ZomeName, EntryDefId, EntryVisibility};
    use holochain_serialized_bytes::SerializedBytes;
    use holochain_types::prelude::{DnaModifiers, DnaWasm};
    use holochain_zome_types::zome::{CoordinatorZomeDef, IntegrityZomeDef};

    /// Helper to create a test database
    async fn test_db() -> DbWrite<Wasm> {
        test_setup_holochain_data(Wasm)
            .await
            .expect("Failed to create test database")
    }

    #[tokio::test]
    async fn wasm_roundtrip() {
        let db = test_db().await;

        // Create test WASM bytecode
        let code = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]; // WASM magic bytes
        let wasm = DnaWasm {
            code: code.clone().into(),
        };
        let hash = wasm.to_hash().await;
        let wasm_with_hash = DnaWasmHashed::with_pre_hashed(wasm, hash.clone());

        // Should not exist initially
        assert!(!db.as_ref().wasm_exists(&hash).await.unwrap());
        assert!(db.as_ref().get_wasm(&hash).await.unwrap().is_none());

        // Store WASM
        db.put_wasm(wasm_with_hash.clone()).await.unwrap();

        // Should exist now
        assert!(db.as_ref().wasm_exists(&hash).await.unwrap());

        // Retrieve and verify
        let retrieved = db.as_ref().get_wasm(&hash).await.unwrap().unwrap();
        assert_eq!(retrieved.as_hash(), &hash);
        assert_eq!(retrieved.as_content().code.as_ref(), code.as_slice());
    }

    #[tokio::test]
    async fn dna_def_roundtrip() {
        let db = test_db().await;

        // Create test DNA definition
        let mut integrity_zomes = Vec::new();
        let integrity_code = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let integrity_wasm = DnaWasm {
            code: integrity_code.into(),
        };
        let integrity_hash = integrity_wasm.to_hash().await;
        let integrity_wasm_hashed =
            DnaWasmHashed::with_pre_hashed(integrity_wasm, integrity_hash.clone());

        // Store the WASM first
        db.put_wasm(integrity_wasm_hashed).await.unwrap();

        integrity_zomes.push((
            ZomeName::from("integrity_zome"),
            IntegrityZomeDef::from_hash(integrity_hash),
        ));

        let mut coordinator_zomes = Vec::new();
        let coordinator_code = vec![0x00, 0x61, 0x73, 0x6d, 0x02, 0x00, 0x00, 0x00];
        let coordinator_wasm = DnaWasm {
            code: coordinator_code.into(),
        };
        let coordinator_hash = coordinator_wasm.to_hash().await;
        let coordinator_wasm_hashed =
            DnaWasmHashed::with_pre_hashed(coordinator_wasm, coordinator_hash.clone());

        // Store the WASM first
        db.put_wasm(coordinator_wasm_hashed).await.unwrap();

        coordinator_zomes.push((
            ZomeName::from("coordinator_zome"),
            CoordinatorZomeDef::from_hash(coordinator_hash),
        ));

        let dna_def = DnaDef {
            name: "test_dna".to_string(),
            modifiers: DnaModifiers {
                network_seed: "test_seed".to_string(),
                properties: SerializedBytes::default(),
            },
            integrity_zomes,
            coordinator_zomes,
            #[cfg(feature = "unstable-migration")]
            lineage: std::collections::HashSet::new(),
        };

        let hash = dna_def.to_hash();

        // Should not exist initially
        assert!(!db.as_ref().dna_def_exists(&hash).await.unwrap());
        assert!(db.as_ref().get_dna_def(&hash).await.unwrap().is_none());

        // Store DNA definition
        db.put_dna_def(&dna_def).await.unwrap();

        // Should exist now
        assert!(db.as_ref().dna_def_exists(&hash).await.unwrap());

        // Retrieve and verify
        let retrieved = db.as_ref().get_dna_def(&hash).await.unwrap().unwrap();
        assert_eq!(retrieved.name, "test_dna");
        assert_eq!(retrieved.modifiers.network_seed, "test_seed");
        assert_eq!(retrieved.integrity_zomes.len(), 1);
        assert_eq!(retrieved.coordinator_zomes.len(), 1);

        // Verify zome names
        assert!(retrieved
            .integrity_zomes
            .iter()
            .any(|(name, _)| name == &ZomeName::from("integrity_zome")));
        assert!(retrieved
            .coordinator_zomes
            .iter()
            .any(|(name, _)| name == &ZomeName::from("coordinator_zome")));
    }

    #[tokio::test]
    async fn entry_def_roundtrip() {
        let db = test_db().await;

        // Create test entry definitions
        let key1 = vec![1, 2, 3, 4];
        let entry_def1 = EntryDef {
            id: EntryDefId::App("test_entry".into()),
            visibility: EntryVisibility::Public,
            required_validations: 5u8.into(),
            cache_at_agent_activity: false,
        };

        let key2 = vec![5, 6, 7, 8];
        let entry_def2 = EntryDef {
            id: EntryDefId::CapGrant,
            visibility: EntryVisibility::Private,
            required_validations: 3u8.into(),
            cache_at_agent_activity: false,
        };

        // Should not exist initially
        assert!(!db.as_ref().entry_def_exists(&key1).await.unwrap());
        assert!(db.as_ref().get_entry_def(&key1).await.unwrap().is_none());

        // Store entry definitions
        db.put_entry_def(key1.clone(), &entry_def1).await.unwrap();
        db.put_entry_def(key2.clone(), &entry_def2).await.unwrap();

        // Should exist now
        assert!(db.as_ref().entry_def_exists(&key1).await.unwrap());
        assert!(db.as_ref().entry_def_exists(&key2).await.unwrap());

        // Retrieve and verify entry_def1
        let retrieved1 = db.as_ref().get_entry_def(&key1).await.unwrap().unwrap();
        assert_eq!(retrieved1.id, EntryDefId::App("test_entry".into()));
        assert_eq!(retrieved1.visibility, EntryVisibility::Public);
        assert_eq!(u8::from(retrieved1.required_validations), 5);

        // Retrieve and verify entry_def2
        let retrieved2 = db.as_ref().get_entry_def(&key2).await.unwrap().unwrap();
        assert_eq!(retrieved2.id, EntryDefId::CapGrant);
        assert_eq!(retrieved2.visibility, EntryVisibility::Private);
        assert_eq!(u8::from(retrieved2.required_validations), 3);

        // Test get_all_entry_defs
        let all_defs = db.as_ref().get_all_entry_defs().await.unwrap();
        assert_eq!(all_defs.len(), 2);

        // Verify both entries are present (order may vary)
        let keys: Vec<_> = all_defs.iter().map(|(k, _)| k.clone()).collect();
        assert!(keys.contains(&key1));
        assert!(keys.contains(&key2));
    }

    #[tokio::test]
    async fn dna_def_with_dependencies() {
        let db = test_db().await;

        // Create WASM for zomes
        let wasm_code = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let wasm = DnaWasm {
            code: wasm_code.into(),
        };
        let wasm_hash = wasm.to_hash().await;
        let wasm_hashed = DnaWasmHashed::with_pre_hashed(wasm, wasm_hash.clone());
        db.put_wasm(wasm_hashed).await.unwrap();

        // Create integrity zome with dependencies
        let mut integrity_zomes = Vec::new();
        let integrity_def = IntegrityZomeDef::from_hash(wasm_hash.clone());
        integrity_zomes.push((ZomeName::from("base_integrity"), integrity_def));

        // Create coordinator zome with dependencies on integrity zome
        let mut coordinator_zomes = Vec::new();
        let coordinator_def = CoordinatorZomeDef::from_hash(wasm_hash.clone());
        coordinator_zomes.push((ZomeName::from("coordinator"), coordinator_def));

        let dna_def = DnaDef {
            name: "test_dna_deps".to_string(),
            modifiers: DnaModifiers {
                network_seed: "seed".to_string(),
                properties: SerializedBytes::default(),
            },
            integrity_zomes,
            coordinator_zomes,
            #[cfg(feature = "unstable-migration")]
            lineage: std::collections::HashSet::new(),
        };

        let hash = dna_def.to_hash();

        // Store and retrieve
        db.put_dna_def(&dna_def).await.unwrap();
        let retrieved = db.as_ref().get_dna_def(&hash).await.unwrap().unwrap();

        // Verify structure
        assert_eq!(retrieved.name, "test_dna_deps");
        assert_eq!(retrieved.integrity_zomes.len(), 1);
        assert_eq!(retrieved.coordinator_zomes.len(), 1);
    }

    #[tokio::test]
    async fn entry_def_all_types() {
        let db = test_db().await;

        // Test all EntryDefId variants
        let app_key = vec![1];
        let app_entry = EntryDef {
            id: EntryDefId::App("my_app_entry".into()),
            visibility: EntryVisibility::Public,
            required_validations: 5u8.into(),
            cache_at_agent_activity: false,
        };

        let cap_claim_key = vec![2];
        let cap_claim_entry = EntryDef {
            id: EntryDefId::CapClaim,
            visibility: EntryVisibility::Private,
            required_validations: 3u8.into(),
            cache_at_agent_activity: false,
        };

        let cap_grant_key = vec![3];
        let cap_grant_entry = EntryDef {
            id: EntryDefId::CapGrant,
            visibility: EntryVisibility::Public,
            required_validations: 2u8.into(),
            cache_at_agent_activity: false,
        };

        // Store all types
        db.put_entry_def(app_key.clone(), &app_entry).await.unwrap();
        db.put_entry_def(cap_claim_key.clone(), &cap_claim_entry)
            .await
            .unwrap();
        db.put_entry_def(cap_grant_key.clone(), &cap_grant_entry)
            .await
            .unwrap();

        // Retrieve and verify each type
        let retrieved_app = db.as_ref().get_entry_def(&app_key).await.unwrap().unwrap();
        assert!(matches!(retrieved_app.id, EntryDefId::App(_)));
        assert_eq!(retrieved_app.visibility, EntryVisibility::Public);

        let retrieved_cap_claim = db
            .as_ref()
            .get_entry_def(&cap_claim_key)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(retrieved_cap_claim.id, EntryDefId::CapClaim));
        assert_eq!(retrieved_cap_claim.visibility, EntryVisibility::Private);

        let retrieved_cap_grant = db
            .as_ref()
            .get_entry_def(&cap_grant_key)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(retrieved_cap_grant.id, EntryDefId::CapGrant));
        assert_eq!(retrieved_cap_grant.visibility, EntryVisibility::Public);

        // Verify all are in get_all
        let all = db.as_ref().get_all_entry_defs().await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn update_dna_def_removes_orphaned_zomes() {
        let db = test_db().await;

        // Create WASM for zomes
        let wasm_code = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let wasm = DnaWasm {
            code: wasm_code.into(),
        };
        let wasm_hash = wasm.to_hash().await;
        let wasm_hashed = DnaWasmHashed::with_pre_hashed(wasm, wasm_hash.clone());
        db.put_wasm(wasm_hashed).await.unwrap();

        // Create initial DNA with 3 integrity zomes and 2 coordinator zomes
        let integrity_zomes = vec![
            (
                ZomeName::from("integrity1"),
                IntegrityZomeDef::from_hash(wasm_hash.clone()),
            ),
            (
                ZomeName::from("integrity2"),
                IntegrityZomeDef::from_hash(wasm_hash.clone()),
            ),
            (
                ZomeName::from("integrity3"),
                IntegrityZomeDef::from_hash(wasm_hash.clone()),
            ),
        ];

        let coordinator_zomes = vec![
            (
                ZomeName::from("coordinator1"),
                CoordinatorZomeDef::from_hash(wasm_hash.clone()),
            ),
            (
                ZomeName::from("coordinator2"),
                CoordinatorZomeDef::from_hash(wasm_hash.clone()),
            ),
        ];

        let dna_def_v1 = DnaDef {
            name: "test_update".to_string(),
            modifiers: DnaModifiers {
                network_seed: "seed".to_string(),
                properties: SerializedBytes::default(),
            },
            integrity_zomes,
            coordinator_zomes,
            #[cfg(feature = "unstable-migration")]
            lineage: std::collections::HashSet::new(),
        };

        let hash = dna_def_v1.to_hash();
        db.put_dna_def(&dna_def_v1).await.unwrap();

        // Verify initial state
        let retrieved_v1 = db.as_ref().get_dna_def(&hash).await.unwrap().unwrap();
        assert_eq!(retrieved_v1.integrity_zomes.len(), 3);
        assert_eq!(retrieved_v1.coordinator_zomes.len(), 2);

        // Count zomes directly in the database
        let integrity_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM IntegrityZome WHERE dna_hash = ?")
                .bind(hash.get_raw_39())
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(integrity_count, 3);

        let coordinator_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM CoordinatorZome WHERE dna_hash = ?")
                .bind(hash.get_raw_39())
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(coordinator_count, 2);

        // Update DNA with fewer zomes (1 integrity, 1 coordinator)
        let integrity_zomes_v2 = vec![(
            ZomeName::from("integrity1"),
            IntegrityZomeDef::from_hash(wasm_hash.clone()),
        )];

        let coordinator_zomes_v2 = vec![(
            ZomeName::from("coordinator1"),
            CoordinatorZomeDef::from_hash(wasm_hash.clone()),
        )];

        let dna_def_v2 = DnaDef {
            name: "test_update_v2".to_string(),
            modifiers: DnaModifiers {
                network_seed: "seed_v2".to_string(),
                properties: SerializedBytes::default(),
            },
            integrity_zomes: integrity_zomes_v2,
            coordinator_zomes: coordinator_zomes_v2,
            #[cfg(feature = "unstable-migration")]
            lineage: std::collections::HashSet::new(),
        };

        // Different hash since zomes changed
        let hash_v2 = dna_def_v2.to_hash();
        assert_ne!(hash, hash_v2, "Hash should change when zomes change");

        // Update the DNA definition
        db.put_dna_def(&dna_def_v2).await.unwrap();

        // Verify old DNA still has original zomes
        let retrieved_v1 = db.as_ref().get_dna_def(&hash).await.unwrap().unwrap();
        assert_eq!(retrieved_v1.integrity_zomes.len(), 3);
        assert_eq!(retrieved_v1.coordinator_zomes.len(), 2);

        // Verify new DNA has new zomes
        let retrieved_v2 = db.as_ref().get_dna_def(&hash_v2).await.unwrap().unwrap();
        assert_eq!(retrieved_v2.integrity_zomes.len(), 1);
        assert_eq!(retrieved_v2.coordinator_zomes.len(), 1);

        // Verify no orphaned zomes remain for the old DNA hash
        let integrity_count_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM IntegrityZome WHERE dna_hash = ?")
                .bind(hash.get_raw_39())
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(
            integrity_count_after, 3,
            "Original DNA should still have 3 integrity zomes"
        );

        let coordinator_count_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM CoordinatorZome WHERE dna_hash = ?")
                .bind(hash.get_raw_39())
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(
            coordinator_count_after, 2,
            "Original DNA should still have 2 coordinator zomes"
        );

        // Verify new DNA has correct counts
        let new_integrity_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM IntegrityZome WHERE dna_hash = ?")
                .bind(hash_v2.get_raw_39())
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(
            new_integrity_count, 1,
            "New DNA should have 1 integrity zome"
        );

        let new_coordinator_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM CoordinatorZome WHERE dna_hash = ?")
                .bind(hash_v2.get_raw_39())
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(
            new_coordinator_count, 1,
            "New DNA should have 1 coordinator zome"
        );
    }
}
