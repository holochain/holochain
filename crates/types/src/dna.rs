//! dna is a library for working with holochain dna files/entries.
//!
//! It includes utilities for representing dna structures in memory,
//! as well as serializing and deserializing dna, mainly to json format.

pub mod error;
pub mod wasm;
pub mod zome;
use crate::prelude::*;
use derive_more::From;
pub use error::DnaError;
use holo_hash::impl_hashable_content;
pub use holo_hash::*;
use holochain_zome_types::zome::ZomeName;
use std::collections::BTreeMap;

use self::error::DnaResult;

/// Zomes need to be an ordered map from ZomeName to a Zome
pub type Zomes = Vec<(ZomeName, zome::Zome)>;

/// A type to allow json values to be used as [SerializedBytes]
#[derive(Debug, Clone, From, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct JsonProperties(serde_json::Value);

impl JsonProperties {
    /// Create new properties from json value
    pub fn new(properties: serde_json::Value) -> Self {
        JsonProperties(properties)
    }
}

/// Represents the top-level holochain dna object.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, SerializedBytes)]
pub struct DnaDef {
    /// The friendly "name" of a Holochain DNA.
    pub name: String,

    /// A UUID for uniquifying this Dna.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub uuid: String,

    /// Any arbitrary application properties can be included in this object.
    pub properties: SerializedBytes,

    /// An array of zomes associated with your holochain application.
    pub zomes: Zomes,
}

impl DnaDef {
    /// Calculate DnaHash for DnaDef
    pub async fn dna_hash(&self) -> DnaHash {
        DnaHash::with_data(self).await
    }

    /// Return a Zome
    pub fn get_zome(&self, zome_name: &ZomeName) -> Result<&zome::Zome, DnaError> {
        self.zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .map(|(_, zome)| zome)
            .ok_or_else(|| DnaError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
    }
}

/// A DnaDef paired with its DnaHash
pub type DnaDefHashed = HoloHashed<DnaDef>;

impl_hashable_content!(DnaDef, Dna);

/// Wasms need to be an ordered map from WasmHash to a DnaWasm
pub type Wasms = BTreeMap<holo_hash::WasmHash, wasm::DnaWasm>;

/// Represents a full DNA file including WebAssembly bytecode.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, SerializedBytes)]
pub struct DnaFile {
    /// The hashable portion that can be shared with hApp code.
    pub dna: DnaDef,

    /// The hash of `self.dna` converted through `SerializedBytes`.
    /// (This can be a full holo_hash because we never send a `DnaFile` to Wasm.)
    pub dna_hash: holo_hash::DnaHash,

    /// The bytes of the WASM zomes referenced in the Dna portion.
    pub code: Wasms,
}

impl From<DnaFile> for (DnaDef, Vec<wasm::DnaWasm>) {
    fn from(dna_file: DnaFile) -> (DnaDef, Vec<wasm::DnaWasm>) {
        (
            dna_file.dna,
            dna_file.code.into_iter().map(|(_, w)| w).collect(),
        )
    }
}

impl DnaFile {
    /// Construct a new DnaFile instance.
    pub async fn new(
        dna: DnaDef,
        wasm: impl IntoIterator<Item = wasm::DnaWasm>,
    ) -> Result<Self, DnaError> {
        let mut code = BTreeMap::new();
        for wasm in wasm {
            let wasm_hash = holo_hash::WasmHash::with_data(&wasm).await;
            code.insert(wasm_hash, wasm);
        }
        let dna_hash = holo_hash::DnaHash::with_data(&dna).await;
        Ok(Self {
            dna,
            dna_hash,
            code,
        })
    }

    /// Verify that the DNA hash in the file matches the DnaDef
    pub async fn verify_hash(&self) -> Result<(), DnaError> {
        let dna_hash = holo_hash::DnaHash::with_data(&self.dna).await;
        if self.dna_hash == dna_hash {
            Ok(())
        } else {
            Err(DnaError::DnaHashMismatch(self.dna_hash.clone(), dna_hash))
        }
    }

    /// Load dna_file bytecode into this rust struct.
    pub async fn from_file_content(data: &[u8]) -> Result<Self, DnaError> {
        // Not super efficient memory-wise, but doesn't block any threads
        let data = data.to_vec();
        let dna_file = tokio::task::spawn_blocking(move || {
            let mut gz = flate2::read::GzDecoder::new(&data[..]);
            let mut bytes = Vec::new();
            use std::io::Read;
            gz.read_to_end(&mut bytes)?;
            let sb: SerializedBytes = UnsafeBytes::from(bytes).into();
            let dna_file: DnaFile = sb.try_into()?;
            DnaResult::Ok(dna_file)
        })
        .await
        .expect("blocking thread panicked - panicking here too")?;
        dna_file.verify_hash().await?;
        Ok(dna_file)
    }

    /// Transform this DnaFile into a new DnaFile with different properties
    /// and, hence, a different DnaHash.
    pub async fn with_properties(self, properties: SerializedBytes) -> Result<Self, DnaError> {
        let (mut dna, wasm): (DnaDef, Vec<wasm::DnaWasm>) = self.into();
        dna.properties = properties;
        DnaFile::new(dna, wasm).await
    }

    /// Transform this DnaFile into a new DnaFile with a different UUID
    /// and, hence, a different DnaHash.
    pub async fn with_uuid(self, uuid: String) -> Result<Self, DnaError> {
        let (mut dna, wasm): (DnaDef, Vec<wasm::DnaWasm>) = self.into();
        dna.uuid = uuid;
        DnaFile::new(dna, wasm).await
    }

    /// The hashable portion that can be shared with hApp code.
    pub fn dna(&self) -> &DnaDef {
        &self.dna
    }

    /// The hash of the dna def
    pub fn dna_hash(&self) -> &holo_hash::DnaHash {
        &self.dna_hash
    }

    /// The bytes of the WASM zomes referenced in the Dna portion.
    pub fn code(&self) -> &BTreeMap<holo_hash::WasmHash, wasm::DnaWasm> {
        &self.code
    }

    /// Fetch the Webassembly byte code for a zome.
    pub fn get_wasm_for_zome(&self, zome_name: &ZomeName) -> Result<&wasm::DnaWasm, DnaError> {
        let wasm_hash = &self.dna.get_zome(zome_name)?.wasm_hash;
        self.code
            .get(wasm_hash)
            .ok_or_else(|| DnaError::InvalidWasmHash)
    }

    /// Render this dna_file as bytecode to send over the wire, or store in a file.
    pub async fn to_file_content(&self) -> Result<Vec<u8>, DnaError> {
        // Not super efficient memory-wise, but doesn't block any threads
        let dna_file = self.clone();
        // TODO: remove
        dna_file.verify_hash().await.expect("TODO, remove");
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

impl std::fmt::Debug for DnaFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("DnaFile(dna_hash = {})", self.dna_hash))
    }
}
