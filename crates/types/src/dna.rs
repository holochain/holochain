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
use holochain_zome_types::zome::ZomeName;

/// A type to allow json values to be used as [SerializedBtyes]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct Properties {
    properties: serde_json::Value,
}

/// Represents the top-level holochain dna object.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, SerializedBytes)]
pub struct Dna {
    /// The friendly "name" of a Holochain DNA.
    pub name: String,

    /// A UUID for uniquifying this Dna.
    // TODO: consider Vec<u8> instead (https://github.com/Holo-Host/holochain-2020/pull/86#discussion_r412689085)
    pub uuid: String,

    /// Any arbitrary application properties can be included in this object.
    pub properties: SerializedBytes,

    /// An array of zomes associated with your holochain application.
    pub zomes: BTreeMap<ZomeName, zome::Zome>,
}

impl Dna {
    /// Gets DnaHash from Dna
    // FIXME: use async with_data, or consider wrapper type
    // https://github.com/Holo-Host/holochain-2020/pull/86#discussion_r413222920
    pub fn dna_hash(&self) -> DnaHash {
        let sb: SerializedBytes = self.try_into().expect("TODO: can this fail?");
        DnaHash::with_data_sync(&sb.bytes())
    }

    /// Return a Zome
    pub fn get_zome(&self, zome_name: &ZomeName) -> Result<&zome::Zome, DnaError> {
        self.zomes
            .get(zome_name)
            .ok_or_else(|| DnaError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
    }
}

impl Properties {
    /// Create new properties from json value
    pub fn new(properties: serde_json::Value) -> Self {
        Properties { properties }
    }
}
