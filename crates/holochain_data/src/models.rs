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
use holochain_types::prelude::CellId;
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
            hash: hash.get_raw_32().to_vec(),
            code,
        }
    }

    /// Get the WasmHash from this model.
    pub fn wasm_hash(&self) -> WasmHash {
        WasmHash::from_raw_32(self.hash.clone())
    }
}

/// Database model for DNA definition.
///
/// Maps to the `DnaDef` table. This is a flattened version of the
/// `DnaDef` struct from `holochain_zome_types`. Zomes are stored
/// in separate tables.
#[derive(Debug, Clone, FromRow)]
pub struct DnaDefModel {
    /// The hash of the DNA definition (32 bytes).
    pub hash: Vec<u8>,
    /// The agent public key (32 bytes).
    pub agent: Vec<u8>,
    /// The friendly name of the DNA.
    pub name: String,
    /// The network seed for DHT partitioning.
    pub network_seed: String,
    /// Serialized application properties.
    pub properties: Vec<u8>,
    /// DNA lineage for migration support (optional, JSON `HashSet<DnaHash>`)
    pub lineage: Option<sqlx::types::JsonValue>,
}

impl DnaDefModel {
    /// Create a new DnaDefModel.
    pub fn new(
        cell_id: &CellId,
        name: String,
        network_seed: String,
        properties: Vec<u8>,
        lineage: Option<sqlx::types::JsonValue>,
    ) -> Self {
        Self {
            hash: cell_id.dna_hash().get_raw_32().to_vec(),
            agent: cell_id.agent_pubkey().get_raw_32().to_vec(),
            name,
            network_seed,
            properties,
            lineage,
        }
    }

    /// Get the DnaHash from this model.
    pub fn dna_hash(&self) -> DnaHash {
        DnaHash::from_raw_32(self.hash.clone())
    }

    /// Get the AgentPubKey from this model.
    pub fn agent_pubkey(&self) -> holochain_types::prelude::AgentPubKey {
        holochain_types::prelude::AgentPubKey::from_raw_32(self.agent.clone())
    }

    /// Create a CellId from the DNA hash and agent pubkey.
    pub fn to_cell_id(&self) -> CellId {
        let dna_hash = self.dna_hash();
        let agent = self.agent_pubkey();
        CellId::new(dna_hash, agent)
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

        #[cfg(feature = "unstable-migration")]
        let lineage = self
            .lineage
            .as_ref()
            .map(|json_value| serde_json::from_value(json_value.clone()))
            .transpose()
            .map_err(|e: serde_json::Error| e.to_string())?
            .unwrap_or_default();

        Ok(DnaDef {
            name: self.name.clone(),
            modifiers,
            integrity_zomes,
            coordinator_zomes,
            #[cfg(feature = "unstable-migration")]
            lineage,
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
    /// The agent public key (32 bytes).
    pub agent: Vec<u8>,
    /// The index/position of this zome in the DNA.
    pub zome_index: i64,
    /// The name of the zome.
    pub zome_name: String,
    /// The WASM hash for this zome (NULL for inline zomes).
    pub wasm_hash: Option<Vec<u8>>,
    /// List of zome dependency names.
    pub dependencies: sqlx::types::Json<Vec<String>>,
}

impl IntegrityZomeModel {
    /// Create a new IntegrityZomeModel.
    pub fn new(
        cell_id: &CellId,
        zome_index: usize,
        zome_name: String,
        wasm_hash: Option<WasmHash>,
        dependencies: Vec<String>,
    ) -> Self {
        Self {
            dna_hash: cell_id.dna_hash().get_raw_32().to_vec(),
            agent: cell_id.agent_pubkey().get_raw_32().to_vec(),
            zome_index: zome_index as i64,
            zome_name,
            wasm_hash: wasm_hash.map(|h| h.get_raw_32().to_vec()),
            dependencies: sqlx::types::Json(dependencies),
        }
    }

    /// Get the WasmHash from this model, if present.
    pub fn wasm_hash(&self) -> Option<WasmHash> {
        self.wasm_hash
            .as_ref()
            .map(|bytes| WasmHash::from_raw_32(bytes.clone()))
    }

    /// Convert to a tuple suitable for DnaDef construction.
    ///
    /// Returns (ZomeName, IntegrityZomeDef) which can be used in the integrity_zomes Vec.
    pub fn to_zome_tuple(&self) -> Result<(ZomeName, IntegrityZomeDef), String> {
        let zome_name = ZomeName(Cow::Owned(self.zome_name.clone()));
        let dependencies: Vec<ZomeName> = self
            .dependencies
            .0
            .iter()
            .map(|s| ZomeName(Cow::Owned(s.clone())))
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
    /// The agent public key (32 bytes).
    pub agent: Vec<u8>,
    /// The index/position of this zome in the DNA.
    pub zome_index: i64,
    /// The name of the zome.
    pub zome_name: String,
    /// The WASM hash for this zome (NULL for inline zomes).
    pub wasm_hash: Option<Vec<u8>>,
    /// List of zome dependency names.
    pub dependencies: sqlx::types::Json<Vec<String>>,
}

impl CoordinatorZomeModel {
    /// Create a new CoordinatorZomeModel.
    pub fn new(
        cell_id: &CellId,
        zome_index: usize,
        zome_name: String,
        wasm_hash: Option<WasmHash>,
        dependencies: Vec<String>,
    ) -> Self {
        Self {
            dna_hash: cell_id.dna_hash().get_raw_32().to_vec(),
            agent: cell_id.agent_pubkey().get_raw_32().to_vec(),
            zome_index: zome_index as i64,
            zome_name,
            wasm_hash: wasm_hash.map(|h| h.get_raw_32().to_vec()),
            dependencies: sqlx::types::Json(dependencies),
        }
    }

    /// Get the WasmHash from this model, if present.
    pub fn wasm_hash(&self) -> Option<WasmHash> {
        self.wasm_hash
            .as_ref()
            .map(|bytes| WasmHash::from_raw_32(bytes.clone()))
    }

    /// Convert to a tuple suitable for DnaDef construction.
    ///
    /// Returns (ZomeName, CoordinatorZomeDef) which can be used in the coordinator_zomes Vec.
    pub fn to_zome_tuple(&self) -> Result<(ZomeName, CoordinatorZomeDef), String> {
        let zome_name = ZomeName(Cow::Owned(self.zome_name.clone()));
        let dependencies: Vec<ZomeName> = self
            .dependencies
            .0
            .iter()
            .map(|s| ZomeName(Cow::Owned(s.clone())))
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
            .map_err(|e| format!("Invalid required_validations: {e}"))?;
        let required_validations = required_validations_u8.into();

        Ok(EntryDef {
            id,
            visibility,
            required_validations,
            cache_at_agent_activity: false, // Default value as mentioned in AGENTS.md
        })
    }
}
