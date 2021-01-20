//! dna is a library for working with holochain dna files/entries.
//!
//! It includes utilities for representing dna structures in memory,
//! as well as serializing and deserializing dna, mainly to json format.

pub mod error;
pub mod wasm;
pub mod zome;
use crate::prelude::*;
pub use error::DnaError;
use holo_hash::impl_hashable_content;
pub use holo_hash::*;
use holochain_zome_types::ZomeName;
use std::collections::BTreeMap;

/// Zomes need to be an ordered map from ZomeName to a Zome
pub type Zomes = Vec<(ZomeName, zome::ZomeDef)>;

/// A type to allow json values to be used as [SerializedBytes]
#[derive(Debug, Clone, derive_more::From, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct YamlProperties(serde_yaml::Value);

impl YamlProperties {
    /// Create new properties from json value
    pub fn new(properties: serde_yaml::Value) -> Self {
        YamlProperties(properties)
    }
}

/// Represents the top-level holochain dna object.
#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, Eq, SerializedBytes, derive_builder::Builder,
)]
#[builder(public)]
pub struct DnaDef {
    /// The friendly "name" of a Holochain DNA.
    #[builder(default = "\"Generated DnaDef\".to_string()")]
    pub name: String,

    /// A UUID for uniquifying this Dna.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub uuid: String,

    /// Any arbitrary application properties can be included in this object.
    #[builder(default = "().try_into().unwrap()")]
    pub properties: SerializedBytes,

    /// An array of zomes associated with your holochain application.
    pub zomes: Zomes,
}

#[cfg(feature = "test_utils")]
impl DnaDef {
    /// Create a DnaDef with a random UUID, useful for testing
    pub fn unique_from_zomes(zomes: Vec<Zome>) -> Self {
        let zomes = zomes.into_iter().map(|z| z.into_inner()).collect();
        DnaDefBuilder::default()
            .zomes(zomes)
            .random_uuid()
            .build()
            .unwrap()
    }
}

impl DnaDef {
    /// Return a Zome
    pub fn get_zome(&self, zome_name: &ZomeName) -> Result<zome::Zome, DnaError> {
        self.zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .cloned()
            .map(|(name, def)| Zome::new(name, def))
            .ok_or_else(|| DnaError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
    }

    /// Return a Zome, error if not a WasmZome
    pub fn get_wasm_zome(&self, zome_name: &ZomeName) -> Result<&zome::WasmZome, DnaError> {
        self.zomes
            .iter()
            .find(|(name, _)| name == zome_name)
            .map(|(_, def)| def)
            .ok_or_else(|| DnaError::ZomeNotFound(format!("Zome '{}' not found", &zome_name,)))
            .and_then(|def| {
                if let ZomeDef::Wasm(wasm_zome) = def {
                    Ok(wasm_zome)
                } else {
                    Err(DnaError::NonWasmZome(zome_name.clone()))
                }
            })
    }
}

fn random_uuid() -> String {
    nanoid::nanoid!()
}

impl DnaDefBuilder {
    /// Provide a random UUID
    pub fn random_uuid(&mut self) -> &mut Self {
        self.uuid = Some(random_uuid());
        self
    }
}

/// A DnaDef paired with its DnaHash
pub type DnaDefHashed = HoloHashed<DnaDef>;

impl_hashable_content!(DnaDef, Dna);

/// Wasms need to be an ordered map from WasmHash to a wasm::DnaWasm
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    derive_more::AsRef,
    derive_more::From,
    derive_more::IntoIterator,
)]
#[serde(from = "WasmMapSerialized", into = "WasmMapSerialized")]
pub struct WasmMap(BTreeMap<holo_hash::WasmHash, wasm::DnaWasm>);

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
struct WasmMapSerialized(Vec<(holo_hash::WasmHash, wasm::DnaWasm)>);

impl From<WasmMap> for WasmMapSerialized {
    fn from(w: WasmMap) -> Self {
        Self(w.0.into_iter().collect())
    }
}

impl From<WasmMapSerialized> for WasmMap {
    fn from(w: WasmMapSerialized) -> Self {
        Self(w.0.into_iter().collect())
    }
}

/// Represents a full DNA file including WebAssembly bytecode.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, SerializedBytes)]
pub struct DnaFile {
    /// The hashable portion that can be shared with hApp code.
    pub dna: DnaDefHashed,

    /// The bytes of the WASM zomes referenced in the Dna portion.
    pub code: WasmMap,
}

impl From<DnaFile> for (DnaDef, Vec<wasm::DnaWasm>) {
    fn from(dna_file: DnaFile) -> (DnaDef, Vec<wasm::DnaWasm>) {
        (
            dna_file.dna.into_content(),
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
        let dna = DnaDefHashed::from_content(dna).await;
        Ok(Self {
            dna,
            code: code.into(),
        })
    }

    /// The DnaDef along with its hash
    pub fn dna(&self) -> &DnaDefHashed {
        &self.dna
    }

    /// Just the DnaDef
    pub fn dna_def(&self) -> &DnaDef {
        &self.dna
    }

    /// The hash of the DnaDef
    pub fn dna_hash(&self) -> &holo_hash::DnaHash {
        self.dna.as_hash()
    }

    /// Verify that the DNA hash in the file matches the DnaDef
    pub async fn verify_hash(&self) -> Result<(), DnaError> {
        self.dna
            .verify_hash()
            .await
            .map_err(|hash| DnaError::DnaHashMismatch(self.dna.as_hash().clone(), hash))
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

    /// The bytes of the WASM zomes referenced in the Dna portion.
    pub fn code(&self) -> &BTreeMap<holo_hash::WasmHash, wasm::DnaWasm> {
        &self.code.0
    }

    /// Fetch the Webassembly byte code for a zome.
    pub fn get_wasm_for_zome(&self, zome_name: &ZomeName) -> Result<&wasm::DnaWasm, DnaError> {
        let wasm_hash = &self.dna.get_wasm_zome(zome_name)?.wasm_hash;
        self.code.0.get(wasm_hash).ok_or(DnaError::InvalidWasmHash)
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
        f.write_fmt(format_args!("DnaFile(dna = {:?})", self.dna))
    }
}
