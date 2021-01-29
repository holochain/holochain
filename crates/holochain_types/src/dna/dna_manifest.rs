use std::path::PathBuf;

use crate::prelude::*;
use holo_hash::*;
use holochain_zome_types::ZomeName;

/// The structure of data that goes in the DNA bundle manifest,
/// i.e. "dna.yaml"
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DnaManifest {
    /// The friendly "name" of a Holochain DNA.
    pub name: String,

    /// A UUID for uniquifying this Dna.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub uuid: String,

    /// Any arbitrary application properties can be included in this object.
    pub properties: Option<YamlProperties>,

    /// An array of zomes associated with your DNA.
    /// The order is significant: it determines initialization order.
    pub zomes: Vec<ZomeManifest>,
}

/// Manifest for an individual Zome
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ZomeManifest {
    name: ZomeName,
    hash: Option<WasmHashB64>,
    #[serde(flatten)]
    location: ZomeLocation,
}

/// Alias for a suitable representation of zome location
pub type ZomeLocation = mr_bundle::Location;

impl mr_bundle::Manifest for DnaManifest {
    fn locations(&self) -> Vec<mr_bundle::Location> {
        self.zomes
            .iter()
            .map(|zome| zome.location.clone())
            .collect()
    }

    fn path(&self) -> PathBuf {
        "dna.yaml".into()
    }
}
