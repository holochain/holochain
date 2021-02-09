use std::path::PathBuf;

use crate::prelude::*;
use holo_hash::*;
use holochain_zome_types::ZomeName;

/// The structure of data that goes in the DNA bundle manifest,
/// i.e. "dna.yaml"
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DnaManifest {
    /// The friendly "name" of a Holochain DNA.
    pub(crate) name: String,

    /// A UUID for uniquifying this Dna.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub(crate) uuid: Option<String>,

    /// Any arbitrary application properties can be included in this object.
    pub(crate) properties: Option<YamlProperties>,

    /// An array of zomes associated with your DNA.
    /// The order is significant: it determines initialization order.
    pub(crate) zomes: Vec<ZomeManifest>,
}

impl mr_bundle::Manifest for DnaManifest {
    fn locations(&self) -> Vec<mr_bundle::Location> {
        self.zomes
            .iter()
            .map(|zome| zome.location.clone())
            .collect()
    }

    fn path(&self) -> PathBuf {
        Self::relative_path()
    }
}

impl DnaManifest {
    /// The path of the manifest, relative to an unpacked working directory.
    /// The manifest must live at the root and be named "dna.yaml".
    pub fn relative_path() -> PathBuf {
        "dna.yaml".into()
    }
}

/// Manifest for an individual Zome
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ZomeManifest {
    pub(crate) name: ZomeName,
    pub(crate) hash: Option<WasmHashB64>,
    #[serde(flatten)]
    pub(crate) location: ZomeLocation,
}

/// Alias for a suitable representation of zome location
pub type ZomeLocation = mr_bundle::Location;

impl ZomeManifest {
    /// Accessor
    pub fn location(&self) -> &ZomeLocation {
        &self.location
    }
}

// impl From<DnaManifest> for DnaDef {
//     fn from(m: DnaManifest) -> Self {
//         let zomes = m.zomes.into_iter().map(|z| {
//             (z.name, ZomeDef::Wasm(WasmZome::new(z.)))
//         })
//         Self {
//             name: m.name,
//             uuid: m.uuid.unwrap_or_default(),
//             properties: m.uuid.unwrap_or_else(|| ().try_into().unwrap()),
//             zomes
//         }
//     }
// }
