use super::error::DnaError;
use crate::prelude::*;
use holo_hash::*;
use holochain_zome_types::prelude::*;
use std::collections::BTreeMap;
use std::collections::HashMap;

#[cfg(test)]
mod test;

/// Wasms need to be an ordered map from WasmHash to a wasm::DnaWasm
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    derive_more::AsRef,
    derive_more::From,
    derive_more::IntoIterator,
)]
#[serde(from = "WasmMapSerialized", into = "WasmMapSerialized")]
pub struct WasmMap(BTreeMap<holo_hash::WasmHash, wasm::DnaWasm>);

#[cfg(feature = "test_utils")]
impl WasmMap {
    /// Create an iterator over the WasmMap
    pub fn iter(&self) -> impl Iterator<Item = (&holo_hash::WasmHash, &wasm::DnaWasm)> {
        self.0.iter()
    }
}

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
// TODO: Once we remove the `InstallApp` command which accepts a `DnaFile`,
//       we should remove the Serialize impl on this type, and perhaps rename
//       to indicate that this is simply a validated, fully-formed DnaBundle
//       (i.e. all Wasms are bundled and immediately available, not remote.)
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, SerializedBytes, Hash)]
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
    pub async fn new(dna: DnaDef, wasm: impl IntoIterator<Item = DnaWasm>) -> Self {
        let mut code = BTreeMap::new();
        for wasm in wasm {
            let wasm_hash = WasmHash::with_data(&wasm).await;
            code.insert(wasm_hash, wasm);
        }

        let dna = DnaDefHashed::from_content_sync(dna);
        Self {
            dna,
            code: code.into(),
        }
    }

    /// Update coordinator zomes for this dna.
    pub async fn update_coordinators(
        &mut self,
        coordinator_zomes: CoordinatorZomes,
        wasms: Vec<wasm::DnaWasm>,
    ) -> Result<Vec<WasmHash>, DnaError> {
        let dangling_dep = coordinator_zomes.iter().find_map(|(coord_name, def)| {
            def.as_any_zome_def()
                .dependencies()
                .iter()
                .find_map(|zome_name| {
                    (!self.dna.is_integrity_zome(zome_name))
                        .then(|| (zome_name.to_string(), coord_name.to_string()))
                })
        });
        if let Some((dangling_dep, zome_name)) = dangling_dep {
            return Err(DnaError::DanglingZomeDependency(dangling_dep, zome_name));
        }
        // Get the previous coordinators.
        let previous_coordinators = std::mem::replace(
            &mut self.dna.content.coordinator_zomes,
            Vec::with_capacity(0),
        );

        // Save the order they were installed.
        let mut coordinator_order: Vec<_> = previous_coordinators
            .iter()
            .map(|(n, _)| n.clone())
            .collect();

        // Turn into a map.
        let mut coordinators: HashMap<_, _> = previous_coordinators.into_iter().collect();

        let mut old_wasm_hashes = Vec::with_capacity(coordinator_zomes.len());

        // For each new coordinator insert it to the map.
        for (name, def) in coordinator_zomes {
            match coordinators.insert(name.clone(), def) {
                Some(replaced_coordinator) => {
                    // If this is replacing a previous coordinator then
                    // remove the old wasm.
                    let wasm_hash = replaced_coordinator.wasm_hash(&name)?;
                    self.code.0.remove(&wasm_hash);
                    old_wasm_hashes.push(wasm_hash);
                }
                None => {
                    // If this is a brand new coordinator then add it
                    // to the order.
                    coordinator_order.push(name);
                }
            }
        }

        // Insert all the new wasms.
        for wasm in wasms {
            let wasm_hash = holo_hash::WasmHash::with_data(&wasm).await;
            self.code.0.insert(wasm_hash, wasm);
        }

        // Insert all the coordinators in the correct order.
        self.dna.content.coordinator_zomes = coordinator_order
            .into_iter()
            .filter_map(|name| coordinators.remove_entry(&name))
            .collect();

        Ok(old_wasm_hashes)
    }

    /// Construct a DnaFile from its constituent parts
    #[cfg(feature = "test_utils")]
    pub fn from_parts(dna: DnaDefHashed, code: WasmMap) -> Self {
        Self { dna, code }
    }

    /// Split a DnaFile into its constituent parts
    #[cfg(feature = "test_utils")]
    pub fn into_parts(self) -> (DnaDefHashed, WasmMap) {
        (self.dna, self.code)
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

    /// Transform this DnaFile into a new DnaFile with different properties
    /// and, hence, a different DnaHash.
    pub async fn with_properties(self, properties: SerializedBytes) -> Self {
        let (mut dna, wasm): (DnaDef, Vec<wasm::DnaWasm>) = self.into();
        dna.modifiers.properties = properties;
        DnaFile::new(dna, wasm).await
    }

    /// Transform this DnaFile into a new DnaFile with a different network seed
    /// and, hence, a different DnaHash.
    pub async fn with_network_seed(self, network_seed: NetworkSeed) -> Self {
        let (mut dna, wasm): (DnaDef, Vec<wasm::DnaWasm>) = self.into();
        dna.modifiers.network_seed = network_seed;
        DnaFile::new(dna, wasm).await
    }

    /// The bytes of the WASM zomes referenced in the Dna portion.
    pub fn code(&self) -> &BTreeMap<holo_hash::WasmHash, wasm::DnaWasm> {
        &self.code.0
    }

    /// Fetch the Webassembly byte code for a zome.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    pub fn get_wasm_for_zome(&self, zome_name: &ZomeName) -> Result<&wasm::DnaWasm, DnaError> {
        let wasm_hash = self.dna.get_wasm_zome_hash(zome_name)?;
        self.code.0.get(&wasm_hash).ok_or(DnaError::InvalidWasmHash)
    }

    /// Set the DNA's name.
    pub fn set_name(&self, name: String) -> Self {
        let mut clone = self.clone();
        clone.dna = DnaDefHashed::from_content_sync(clone.dna.set_name(name));
        clone
    }

    /// Change the DNA modifiers while leaving the actual DNA code intact.
    pub fn update_modifiers(&self, dna_modifiers: DnaModifiersOpt) -> Self {
        let mut clone = self.clone();
        clone.dna = DnaDefHashed::from_content_sync(clone.dna.update_modifiers(dna_modifiers));
        clone
    }
}

impl std::fmt::Debug for DnaFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("DnaFile(dna = {:?})", self.dna))
    }
}
