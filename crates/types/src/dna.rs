//! dna is a library for working with holochain dna files/entries.
//!
//! It includes utilities for representing dna structures in memory,
//! as well as serializing and deserializing dna, mainly to json format.

pub mod error;
pub mod wasm;
pub mod zome;
use crate::prelude::*;
pub use error::DnaError;
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
    /// Calculate DnaHash for DnaDef
    pub async fn dna_hash(&self) -> DnaHash {
        let sb: SerializedBytes = self.try_into().expect("failed to hash DnaDef");
        DnaHash::with_data(&sb.bytes()).await
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
    dna: DnaDef,

    /// The hash of the dna def
    /// (this can be a full holo_hash because we never send a DnaFile to WASM)
    dna_hash: holo_hash::DnaHash,

    /// The bytes of the WASM zomes referenced in the Dna portion.
    code: BTreeMap<holo_hash_core::WasmHash, wasm::DnaWasm>,
}

impl DnaFile {
    /// Construct a new DnaFile instance.
    pub async fn new(
        dna: DnaDef,
        wasm: impl IntoIterator<Item = wasm::DnaWasm>,
    ) -> Result<Self, DnaError> {
        let mut code = BTreeMap::new();
        for wasm in wasm.into_iter() {
            let wasm_hash = holo_hash::WasmHash::with_data(&wasm.code()).await;
            let wasm_hash: holo_hash_core::WasmHash = wasm_hash.into();
            code.insert(wasm_hash, wasm);
        }
        let dna_sb: SerializedBytes = (&dna).try_into()?;
        let dna_hash = holo_hash::DnaHash::with_data(dna_sb.bytes()).await;
        Ok(Self {
            dna,
            dna_hash,
            code,
        })
    }

    /// Load dna_file bytecode into this rust struct.
    pub async fn from_file_content(data: &[u8]) -> Result<Self, DnaError> {
        // Not super efficient memory-wise, but doesn't block any threads
        let data = data.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut gz = flate2::read::GzDecoder::new(&data[..]);
            let mut bytes = Vec::new();
            use std::io::Read;
            gz.read_to_end(&mut bytes)?;
            let sb: SerializedBytes = UnsafeBytes::from(bytes).into();
            let dna_file: DnaFile = sb.try_into()?;
            Ok(dna_file)
        })
        .await
        .expect("blocking thread panic!d - panicing here too")
    }

    /// The hashable portion that can be shared with hApp code.
    pub fn dna(&self) -> &DnaDef {
        &self.dna
    }

    /// The hash of the dna def
    /// (this can be a full holo_hash because we never send a DnaFile to WASM)
    pub fn dna_hash(&self) -> &holo_hash::DnaHash {
        &self.dna_hash
    }

    /// The bytes of the WASM zomes referenced in the Dna portion.
    pub fn code(&self) -> &BTreeMap<holo_hash_core::WasmHash, wasm::DnaWasm> {
        &self.code
    }

    /// Fetch the Webassembly byte code for a zome.
    pub fn get_wasm_for_zome(&self, zome_name: &str) -> Result<&wasm::DnaWasm, DnaError> {
        let wasm_hash = &self.dna.get_zome(zome_name)?.wasm_hash;
        self.code
            .get(wasm_hash)
            .ok_or_else(|| DnaError::Invalid("wasm not found".to_string()))
    }

    /// Render this dna_file as bytecode to send over the wire, or store in a file.
    pub async fn as_file_content(&self) -> Result<Vec<u8>, DnaError> {
        // Not super efficient memory-wise, but doesn't block any threads
        let dna_file = self.clone();
        tokio::task::spawn_blocking(move || {
            let data: SerializedBytes = dna_file.try_into()?;
            let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            use std::io::Write;
            enc.write_all(data.bytes())?;
            Ok(enc.finish()?)
        })
        .await
        .expect("blocking thread panic!d - panicing here too")
    }
}
