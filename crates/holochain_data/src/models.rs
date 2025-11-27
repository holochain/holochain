//! Database models for Holochain data structures.
//!
//! These models represent the database schema and may differ from the
//! corresponding types in `holochain_types` or `holochain_zome_types`.
//! The models are designed to be flat and easily mappable to SQL tables.

use holo_hash::{DnaHash, WasmHash};
use holochain_integrity_types::{
    zome::ZomeName, AppEntryName, DnaModifiers, EntryDef, EntryDefId, EntryVisibility,
};
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use holochain_zome_types::{
    prelude::DnaDef,
    zome::{CoordinatorZomeDef, IntegrityZomeDef, WasmZome, ZomeDef},
};
use sqlx::FromRow;
use std::borrow::Cow;

/// Database model for WASM bytecode storage.
///
/// Maps to the `Wasm` table.
#[derive(Debug, Clone, FromRow)]
pub struct WasmModel {
    /// The hash of the WASM code.
    pub hash: Vec<u8>,
    /// The actual WASM bytecode.
    pub code: Vec<u8>,
}

impl WasmModel {
    /// Create a new WasmModel from a hash and code bytes.
    pub fn new(hash: WasmHash, code: Vec<u8>) -> Self {
        Self {
            hash: hash.get_raw_39().to_vec(),
            code,
        }
    }

    /// Get the WasmHash from this model.
    pub fn wasm_hash(&self) -> WasmHash {
        WasmHash::from_raw_39(self.hash.clone())
    }
}

/// Database model for DNA definition.
///
/// Maps to the `DnaDef` table. This is a flattened version of the
/// `DnaDef` struct from `holochain_zome_types`. Zomes are stored
/// in separate tables.
#[derive(Debug, Clone, FromRow)]
pub struct DnaDefModel {
    /// The hash of the DNA definition.
    pub hash: Vec<u8>,
    /// The friendly name of the DNA.
    pub name: String,
    /// The network seed for DHT partitioning.
    pub network_seed: String,
    /// Serialized application properties.
    pub properties: Vec<u8>,
}

impl DnaDefModel {
    /// Create a new DnaDefModel.
    pub fn new(hash: DnaHash, name: String, network_seed: String, properties: Vec<u8>) -> Self {
        Self {
            hash: hash.get_raw_39().to_vec(),
            name,
            network_seed,
            properties,
        }
    }

    /// Get the DnaHash from this model.
    pub fn dna_hash(&self) -> DnaHash {
        DnaHash::from_raw_39(self.hash.clone())
    }

    /// Convert to a DnaDef given the associated zomes.
    ///
    /// This requires integrity and coordinator zome models to be provided,
    /// as they are stored in separate tables.
    pub fn to_dna_def(
        &self,
        integrity_zomes: Vec<IntegrityZomeModel>,
        coordinator_zomes: Vec<CoordinatorZomeModel>,
    ) -> Result<DnaDef, String> {
        let modifiers = DnaModifiers {
            network_seed: self.network_seed.clone(),
            properties: SerializedBytes::from(UnsafeBytes::from(self.properties.clone())),
        };

        let integrity_zomes: Result<Vec<_>, _> = integrity_zomes
            .iter()
            .map(|model| model.to_zome_tuple())
            .collect();
        let integrity_zomes = integrity_zomes?;

        let coordinator_zomes: Result<Vec<_>, _> = coordinator_zomes
            .iter()
            .map(|model| model.to_zome_tuple())
            .collect();
        let coordinator_zomes = coordinator_zomes?;

        Ok(DnaDef {
            name: self.name.clone(),
            modifiers,
            integrity_zomes,
            coordinator_zomes,
        })
    }
}

/// Database model for an integrity zome.
///
/// Maps to the `IntegrityZome` table.
#[derive(Debug, Clone, FromRow)]
pub struct IntegrityZomeModel {
    /// The DNA hash this zome belongs to.
    pub dna_hash: Vec<u8>,
    /// The index/position of this zome in the DNA.
    pub zome_index: i64,
    /// The name of the zome.
    pub zome_name: String,
    /// The WASM hash for this zome (NULL for inline zomes).
    pub wasm_hash: Option<Vec<u8>>,
    /// Comma-separated list of zome dependency names.
    pub dependencies: String,
}

impl IntegrityZomeModel {
    /// Create a new IntegrityZomeModel.
    pub fn new(
        dna_hash: DnaHash,
        zome_index: usize,
        zome_name: String,
        wasm_hash: Option<WasmHash>,
        dependencies: Vec<String>,
    ) -> Self {
        Self {
            dna_hash: dna_hash.get_raw_39().to_vec(),
            zome_index: zome_index as i64,
            zome_name,
            wasm_hash: wasm_hash.map(|h| h.get_raw_39().to_vec()),
            dependencies: dependencies.join(","),
        }
    }

    /// Get the DnaHash from this model.
    pub fn dna_hash(&self) -> DnaHash {
        DnaHash::from_raw_39(self.dna_hash.clone())
    }

    /// Get the WasmHash from this model, if present.
    pub fn wasm_hash(&self) -> Option<WasmHash> {
        self.wasm_hash
            .as_ref()
            .map(|bytes| WasmHash::from_raw_39(bytes.clone()))
    }

    /// Parse the dependencies from comma-separated string.
    pub fn parse_dependencies(&self) -> Vec<String> {
        if self.dependencies.is_empty() {
            Vec::new()
        } else {
            self.dependencies
                .split(',')
                .map(|s| s.to_string())
                .collect()
        }
    }

    /// Convert to a tuple suitable for DnaDef construction.
    ///
    /// Returns (ZomeName, IntegrityZomeDef) which can be used in the integrity_zomes Vec.
    pub fn to_zome_tuple(&self) -> Result<(ZomeName, IntegrityZomeDef), String> {
        let zome_name = ZomeName(Cow::Owned(self.zome_name.clone()));
        let dependencies = self.parse_dependencies();
        let dependencies: Vec<ZomeName> = dependencies
            .into_iter()
            .map(|s| ZomeName(Cow::Owned(s)))
            .collect();

        let zome_def = if let Some(wasm_hash) = self.wasm_hash() {
            IntegrityZomeDef::from(ZomeDef::Wasm(WasmZome {
                wasm_hash,
                dependencies,
            }))
        } else {
            // Inline zomes cannot be reconstructed from database
            return Err("Cannot reconstruct inline zomes from database".to_string());
        };

        Ok((zome_name, zome_def))
    }
}

/// Database model for a coordinator zome.
///
/// Maps to the `CoordinatorZome` table.
#[derive(Debug, Clone, FromRow)]
pub struct CoordinatorZomeModel {
    /// The DNA hash this zome belongs to.
    pub dna_hash: Vec<u8>,
    /// The index/position of this zome in the DNA.
    pub zome_index: i64,
    /// The name of the zome.
    pub zome_name: String,
    /// The WASM hash for this zome (NULL for inline zomes).
    pub wasm_hash: Option<Vec<u8>>,
    /// Comma-separated list of zome dependency names.
    pub dependencies: String,
}

impl CoordinatorZomeModel {
    /// Create a new CoordinatorZomeModel.
    pub fn new(
        dna_hash: DnaHash,
        zome_index: usize,
        zome_name: String,
        wasm_hash: Option<WasmHash>,
        dependencies: Vec<String>,
    ) -> Self {
        Self {
            dna_hash: dna_hash.get_raw_39().to_vec(),
            zome_index: zome_index as i64,
            zome_name,
            wasm_hash: wasm_hash.map(|h| h.get_raw_39().to_vec()),
            dependencies: dependencies.join(","),
        }
    }

    /// Get the DnaHash from this model.
    pub fn dna_hash(&self) -> DnaHash {
        DnaHash::from_raw_39(self.dna_hash.clone())
    }

    /// Get the WasmHash from this model, if present.
    pub fn wasm_hash(&self) -> Option<WasmHash> {
        self.wasm_hash
            .as_ref()
            .map(|bytes| WasmHash::from_raw_39(bytes.clone()))
    }

    /// Parse the dependencies from comma-separated string.
    pub fn parse_dependencies(&self) -> Vec<String> {
        if self.dependencies.is_empty() {
            Vec::new()
        } else {
            self.dependencies
                .split(',')
                .map(|s| s.to_string())
                .collect()
        }
    }

    /// Convert to a tuple suitable for DnaDef construction.
    ///
    /// Returns (ZomeName, CoordinatorZomeDef) which can be used in the coordinator_zomes Vec.
    pub fn to_zome_tuple(&self) -> Result<(ZomeName, CoordinatorZomeDef), String> {
        let zome_name = ZomeName(Cow::Owned(self.zome_name.clone()));
        let dependencies = self.parse_dependencies();
        let dependencies: Vec<ZomeName> = dependencies
            .into_iter()
            .map(|s| ZomeName(Cow::Owned(s)))
            .collect();

        let zome_def = if let Some(wasm_hash) = self.wasm_hash() {
            CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
                wasm_hash,
                dependencies,
            }))
        } else {
            // Inline zomes cannot be reconstructed from database
            return Err("Cannot reconstruct inline zomes from database".to_string());
        };

        Ok((zome_name, zome_def))
    }
}

/// Database model for an entry definition.
///
/// Maps to the `EntryDef` table.
#[derive(Debug, Clone, FromRow)]
pub struct EntryDefModel {
    /// The key derived from EntryDefBufferKey (zome + entry_def_position).
    pub key: Vec<u8>,
    /// The entry definition identifier.
    pub entry_def_id: String,
    /// The type of entry definition: 'App', 'CapClaim', or 'CapGrant'.
    pub entry_def_id_type: String,
    /// The visibility: 'Public' or 'Private'.
    pub visibility: String,
    /// The number of validations required.
    pub required_validations: i64,
}

impl EntryDefModel {
    /// Create a new EntryDefModel.
    pub fn new(
        key: Vec<u8>,
        entry_def_id: String,
        entry_def_id_type: String,
        visibility: String,
        required_validations: u8,
    ) -> Self {
        Self {
            key,
            entry_def_id,
            entry_def_id_type,
            visibility,
            required_validations: required_validations as i64,
        }
    }

    /// Create an EntryDefModel from an EntryDef and key.
    pub fn from_entry_def(key: Vec<u8>, entry_def: &EntryDef) -> Self {
        let (entry_def_id, entry_def_id_type) = match &entry_def.id {
            EntryDefId::App(name) => (name.0.to_string(), "App".to_string()),
            EntryDefId::CapClaim => ("CapClaim".to_string(), "CapClaim".to_string()),
            EntryDefId::CapGrant => ("CapGrant".to_string(), "CapGrant".to_string()),
        };

        let visibility = match entry_def.visibility {
            EntryVisibility::Public => "Public".to_string(),
            EntryVisibility::Private => "Private".to_string(),
        };

        Self {
            key,
            entry_def_id,
            entry_def_id_type,
            visibility,
            required_validations: u8::from(entry_def.required_validations) as i64,
        }
    }

    /// Convert to an EntryDef.
    pub fn to_entry_def(&self) -> Result<EntryDef, String> {
        let id = match self.entry_def_id_type.as_str() {
            "App" => EntryDefId::App(AppEntryName(self.entry_def_id.clone().into())),
            "CapClaim" => EntryDefId::CapClaim,
            "CapGrant" => EntryDefId::CapGrant,
            _ => {
                return Err(format!(
                    "Invalid entry_def_id_type: {}",
                    self.entry_def_id_type
                ))
            }
        };

        let visibility = match self.visibility.as_str() {
            "Public" => EntryVisibility::Public,
            "Private" => EntryVisibility::Private,
            _ => return Err(format!("Invalid visibility: {}", self.visibility)),
        };

        let required_validations_u8: u8 = self
            .required_validations
            .try_into()
            .map_err(|e| format!("Invalid required_validations: {}", e))?;
        let required_validations = required_validations_u8.into();

        Ok(EntryDef {
            id,
            visibility,
            required_validations,
            cache_at_agent_activity: false, // Default value as mentioned in AGENTS.md
        })
    }
}
