//! dna is a library for working with holochain dna files/entries.
//!
//! It includes utilities for representing dna structures in memory,
//! as well as serializing and deserializing dna, mainly to json format.

pub mod error;
pub mod wasm;
pub mod zome;
use crate::prelude::*;
use error::DnaError;
pub use holo_hash::*;
use std::collections::BTreeMap;
/// A type to allow json values to be used as [SerializedBtyes]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct Properties {
    properties: serde_json::Value,
}

impl Properties {
    /// Create new properties from json value
    pub fn new(properties: serde_json::Value) -> Self {
        Properties { properties }
    }
}

/// Represents the top-level holochain dna object.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, SerializedBytes)]
pub struct DnaDef {
    /// The friendly "name" of a Holochain DNA.
    pub name: String,

    /// A UUID for uniquifying this Dna.
    // TODO: consider Vec<u8> instead (https://github.com/Holo-Host/holochain-2020/pull/86#discussion_r412689085)
    pub uuid: String,

    /// Any arbitrary application properties can be included in this object.
    pub properties: SerializedBytes,

    /// An array of zomes associated with your holochain application.
    pub zomes: BTreeMap<String, zome::Zome>,
}

impl DnaDef {
    /// Gets DnaHash from Dna
    // FIXME: use async with_data, or consider wrapper type
    // https://github.com/Holo-Host/holochain-2020/pull/86#discussion_r413222920
    pub fn dna_hash(&self) -> DnaHash {
        let sb: SerializedBytes = self.try_into().expect("TODO: can this fail?");
        DnaHash::with_data_sync(&sb.bytes())
    }

    /// Return a Zome
    pub fn get_zome(&self, zome_name: &str) -> Result<&zome::Zome, DnaError> {
        self.zomes
            .get(zome_name)
            .ok_or_else(|| DnaError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
    }
}

/// Represents a full dna file including Webassembly bytecode.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, SerializedBytes)]
pub struct DnaFile {
    /// The hashable portion that can be shared with hApp code.
    pub dna: DnaDef,

    /// The hash of the dna def
    /// (this can be a full holo_hash because we never send a DnaFile to WASM)
    pub dna_hash: holo_hash::DnaHash,

    /// The bytes of the WASM zomes referenced in the Dna portion.
    pub code: BTreeMap<holo_hash_core::WasmHash, wasm::DnaWasm>,
}

impl DnaFile {
    /// Fetch the Webassembly byte code for a zome.
    pub fn get_wasm_for_zome(&self, zome_name: &str) -> Result<&wasm::DnaWasm, DnaError> {
        let wasm_hash = &self.dna.get_zome(zome_name)?.wasm_hash;
        self.code
            .get(wasm_hash)
            .ok_or_else(|| DnaError::Invalid("wasm not found".to_string()))
    }
}
