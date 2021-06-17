use super::error::DnaError;
use crate::prelude::*;
use holo_hash::*;
use holochain_zome_types::ZomeName;
use std::collections::BTreeMap;

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

/// Represents a full DNA, including DnaDef and WebAssembly bytecode.
///
/// Historical note: This struct was written before `DnaBundle` was introduced.
/// This used to be our file representation of a full distributable DNA.
/// That function has been superseded by `DnaBundle`, but we use this type
/// widely, so there is simply a way to convert from `DnaBundle` to `DnaFile`.
///
/// TODO: Once we remove the `InstallApp` command which accepts a `DnaFile`,
///       we should remove the Serialize impl on this type, and perhaps rename
///       to indicate that this is simply a validated, fully-formed DnaBundle
///       (i.e. all Wasms are bundled and immediately available, not remote.)
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, SerializedBytes)]
pub struct DnaFile {
    /// The hashable portion that can be shared with hApp code.
    pub(super) dna: DnaDefHashed,

    /// The bytes of the WASM zomes referenced in the Dna portion.
    pub(super) code: WasmMap,
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
        let dna = DnaDefHashed::from_content_sync(dna);
        Ok(Self {
            dna,
            code: code.into(),
        })
    }

    /// Construct a DnaFile from its constituent parts
    #[cfg(feature = "fixturators")]
    pub fn from_parts(dna: DnaDefHashed, code: WasmMap) -> Self {
        Self { dna, code }
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
    pub fn verify_hash(&self) -> Result<(), DnaError> {
        self.dna
            .verify_hash_sync()
            .map_err(|hash| DnaError::DnaHashMismatch(self.dna.as_hash().clone(), hash))
    }

    /// Load dna_file bytecode into this rust struct.
    #[deprecated = "remove after app bundles become standard; use DnaBundle instead"]
    pub async fn from_file_content(data: &[u8]) -> Result<Self, DnaError> {
        // Not super efficient memory-wise, but doesn't block any threads
        let data = data.to_vec();
        // Block because gzipping could take some time
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
        dna_file.verify_hash()?;
        Ok(dna_file)
    }

    /// Transform this DnaFile into a new DnaFile with different properties
    /// and, hence, a different DnaHash.
    pub async fn with_properties(self, properties: SerializedBytes) -> Result<Self, DnaError> {
        let (mut dna, wasm): (DnaDef, Vec<wasm::DnaWasm>) = self.into();
        dna.properties = properties;
        DnaFile::new(dna, wasm).await
    }

    /// Transform this DnaFile into a new DnaFile with a different UID
    /// and, hence, a different DnaHash.
    pub async fn with_uid(self, uid: String) -> Result<Self, DnaError> {
        let (mut dna, wasm): (DnaDef, Vec<wasm::DnaWasm>) = self.into();
        dna.uid = uid;
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

    #[deprecated = "remove after app bundles become standard; use DnaBundle instead"]
    /// Render this dna_file as bytecode to send over the wire, or store in a file.
    pub async fn to_file_content(&self) -> Result<Vec<u8>, DnaError> {
        // Not super efficient memory-wise, but doesn't block any threads
        let dna_file = self.clone();
        dna_file.verify_hash()?;
        // Block because gzipping could take some time
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

    /// Change the "phenotype" of this DNA -- the UID and properties -- while
    /// leaving the "genotype" of actual DNA code intact
    pub fn modify_phenotype(&self, uid: Uid, properties: YamlProperties) -> DnaResult<Self> {
        let mut clone = self.clone();
        clone.dna = DnaDefHashed::from_content_sync(
            clone.dna.modify_phenotype(uid, properties.try_into()?),
        );
        Ok(clone)
    }
}

impl std::fmt::Debug for DnaFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("DnaFile(dna = {:?})", self.dna))
    }
}
