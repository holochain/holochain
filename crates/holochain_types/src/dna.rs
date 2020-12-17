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

use self::error::DnaResult;
use self::zome::Zome;
use self::zome::ZomeDef;

#[cfg(feature = "test_utils")]
use self::zome::inline_zome::InlineZome;

/// Zomes need to be an ordered map from ZomeName to a Zome
pub type Zomes = Vec<(ZomeName, zome::ZomeDef)>;

/// A type to allow json values to be used as [SerializedBytes]
#[derive(Debug, Clone, derive_more::From, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct JsonProperties(serde_json::Value);

impl JsonProperties {
    /// Create new properties from json value
    pub fn new(properties: serde_json::Value) -> Self {
        JsonProperties(properties)
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
pub type Wasms = BTreeMap<holo_hash::WasmHash, wasm::DnaWasm>;

/// Represents a full DNA file including WebAssembly bytecode.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, SerializedBytes)]
pub struct DnaFile {
    /// The hashable portion that can be shared with hApp code.
    pub dna: DnaDefHashed,

    /// The bytes of the WASM zomes referenced in the Dna portion.
    pub code: Wasms,
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
        Ok(Self { dna, code })
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
        &self.code
    }

    /// Fetch the Webassembly byte code for a zome.
    pub fn get_wasm_for_zome(&self, zome_name: &ZomeName) -> Result<&wasm::DnaWasm, DnaError> {
        let wasm_hash = &self.dna.get_wasm_zome(zome_name)?.wasm_hash;
        self.code.get(wasm_hash).ok_or(DnaError::InvalidWasmHash)
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

#[cfg(feature = "test_utils")]
impl DnaFile {
    /// Create a DnaFile from a collection of Zomes
    pub async fn from_zomes(
        uuid: String,
        zomes: Vec<(ZomeName, ZomeDef)>,
        wasms: Vec<wasm::DnaWasm>,
    ) -> DnaResult<(Self, Vec<Zome>)> {
        let dna_def = DnaDefBuilder::default()
            .uuid(uuid)
            .zomes(zomes.clone())
            .build()
            .unwrap();

        let dna_file = DnaFile::new(dna_def, wasms).await?;
        let zomes: Vec<Zome> = zomes.into_iter().map(|(n, z)| Zome::new(n, z)).collect();
        Ok((dna_file, zomes))
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm),
    /// with a random UUID
    pub async fn unique_from_zomes(
        zomes: Vec<(ZomeName, ZomeDef)>,
        wasms: Vec<wasm::DnaWasm>,
    ) -> DnaResult<(Self, Vec<Zome>)> {
        Self::from_zomes(random_uuid(), zomes, wasms).await
    }

    /// Create a DnaFile from a collection of TestWasm
    pub async fn from_test_wasms<W>(
        uuid: String,
        test_wasms: Vec<W>,
    ) -> DnaResult<(Self, Vec<Zome>)>
    where
        W: Into<(ZomeName, ZomeDef)> + Into<wasm::DnaWasm> + Clone,
    {
        let zomes = test_wasms.clone().into_iter().map(Into::into).collect();
        let wasms = test_wasms.into_iter().map(Into::into).collect();
        Self::from_zomes(uuid, zomes, wasms).await
    }

    /// Create a DnaFile from a collection of TestWasm
    /// with a random UUID
    pub async fn unique_from_test_wasms<W>(test_wasms: Vec<W>) -> DnaResult<(Self, Vec<Zome>)>
    where
        W: Into<(ZomeName, ZomeDef)> + Into<wasm::DnaWasm> + Clone,
    {
        Self::from_test_wasms(random_uuid(), test_wasms).await
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm)
    pub async fn from_inline_zomes(
        uuid: String,
        zomes: Vec<(&str, InlineZome)>,
    ) -> DnaResult<(Self, Vec<Zome>)> {
        Self::from_zomes(
            uuid,
            zomes
                .into_iter()
                .map(|(n, z)| (n.into(), z.into()))
                .collect(),
            Vec::new(),
        )
        .await
    }

    /// Create a DnaFile from a collection of InlineZomes (no Wasm),
    /// with a random UUID
    pub async fn unique_from_inline_zomes(
        zomes: Vec<(&str, InlineZome)>,
    ) -> DnaResult<(Self, Vec<Zome>)> {
        Self::from_inline_zomes(random_uuid(), zomes).await
    }

    /// Create a DnaFile from a single InlineZome (no Wasm)
    pub async fn from_inline_zome(
        uuid: String,
        zome_name: &str,
        zome: InlineZome,
    ) -> DnaResult<(Self, Zome)> {
        let (dna_file, mut zomes) = Self::from_inline_zomes(uuid, vec![(zome_name, zome)]).await?;
        Ok((dna_file, zomes.pop().unwrap()))
    }

    /// Create a DnaFile from a single InlineZome (no Wasm)
    /// with a random UUID
    pub async fn unique_from_inline_zome(
        zome_name: &str,
        zome: InlineZome,
    ) -> DnaResult<(Self, Zome)> {
        Self::from_inline_zome(random_uuid(), zome_name, zome).await
    }
}

impl std::fmt::Debug for DnaFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("DnaFile(dna_hash = {})", self.dna_hash()))
    }
}
