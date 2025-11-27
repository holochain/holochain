//! Operations for the Wasm database.
//!
//! The Wasm database stores DNA definitions, WASM bytecode, and entry definitions.

use crate::handles::{DbRead, DbWrite};
use crate::kind::Wasm;
use crate::models::{CoordinatorZomeModel, DnaDefModel, EntryDefModel, IntegrityZomeModel, WasmModel};
use holo_hash::{DnaHash, WasmHash};
use holo_hash::HashableContentExtSync;
use holochain_integrity_types::prelude::EntryDef;
use holochain_types::prelude::{DnaDef, DnaWasmHashed};
use holochain_zome_types::zome::{ZomeDef, WasmZome};

// Read operations
impl DbRead<Wasm> {
    /// Check if WASM bytecode exists in the database.
    pub async fn wasm_exists(&self, hash: &WasmHash) -> sqlx::Result<bool> {
        let hash_bytes = hash.get_raw_39();
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM Wasm WHERE hash = ?)")
                .bind(hash_bytes)
                .fetch_one(self.pool())
                .await?;
        Ok(exists)
    }

    /// Get WASM bytecode by hash.
    pub async fn get_wasm(&self, hash: &WasmHash) -> sqlx::Result<Option<DnaWasmHashed>> {
        let hash_bytes = hash.get_raw_39();
        let model: Option<WasmModel> =
            sqlx::query_as("SELECT hash, code FROM Wasm WHERE hash = ?")
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
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM DnaDef WHERE hash = ?)")
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
            "SELECT hash, name, network_seed, properties FROM DnaDef WHERE hash = ?",
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
            Some(model) => model.to_entry_def()
                .map(Some)
                .map_err(|e| sqlx::Error::Decode(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))),
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
                model.to_entry_def()
                    .map(|entry_def| (key, entry_def))
                    .map_err(|e| sqlx::Error::Decode(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))
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

        // Insert DnaDef
        sqlx::query(
            "INSERT OR REPLACE INTO DnaDef (hash, name, network_seed, properties) VALUES (?, ?, ?, ?)",
        )
        .bind(hash_bytes)
        .bind(name)
        .bind(network_seed)
        .bind(properties)
        .execute(&mut *tx)
        .await?;

        // Insert integrity zomes
        for (zome_index, (zome_name, zome_def)) in dna_def.integrity_zomes.iter().enumerate() {
            let wasm_hash = zome_def.wasm_hash(zome_name)
                .map_err(|e| sqlx::Error::Encode(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))))?;
            let wasm_hash_bytes = wasm_hash.get_raw_39();

            // Extract dependencies from the ZomeDef
            let dependencies = match zome_def.as_any_zome_def() {
                ZomeDef::Wasm(WasmZome { dependencies, .. }) => {
                    dependencies.iter().map(|n| n.0.as_ref()).collect::<Vec<_>>().join(",")
                },
                ZomeDef::Inline { dependencies, .. } => {
                    dependencies.iter().map(|n| n.0.as_ref()).collect::<Vec<_>>().join(",")
                },
            };

            sqlx::query(
                "INSERT OR REPLACE INTO IntegrityZome (dna_hash, zome_index, zome_name, wasm_hash, dependencies) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(hash_bytes)
            .bind(zome_index as i64)
            .bind(&zome_name.0)
            .bind(wasm_hash_bytes)
            .bind(dependencies)
            .execute(&mut *tx)
            .await?;
        }

        // Insert coordinator zomes
        for (zome_index, (zome_name, zome_def)) in dna_def.coordinator_zomes.iter().enumerate() {
            let wasm_hash = zome_def.wasm_hash(zome_name)
                .map_err(|e| sqlx::Error::Encode(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))))?;
            let wasm_hash_bytes = wasm_hash.get_raw_39();

            // Extract dependencies from the ZomeDef
            let dependencies = match zome_def.as_any_zome_def() {
                ZomeDef::Wasm(WasmZome { dependencies, .. }) => {
                    dependencies.iter().map(|n| n.0.as_ref()).collect::<Vec<_>>().join(",")
                },
                ZomeDef::Inline { dependencies, .. } => {
                    dependencies.iter().map(|n| n.0.as_ref()).collect::<Vec<_>>().join(",")
                },
            };

            sqlx::query(
                "INSERT OR REPLACE INTO CoordinatorZome (dna_hash, zome_index, zome_name, wasm_hash, dependencies) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(hash_bytes)
            .bind(zome_index as i64)
            .bind(&zome_name.0)
            .bind(wasm_hash_bytes)
            .bind(dependencies)
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
