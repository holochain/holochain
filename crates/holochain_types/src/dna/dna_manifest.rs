use std::path::PathBuf;

use crate::prelude::*;
use holo_hash::*;
use holochain_zome_types::ZomeName;

/// The structure of data that goes in the DNA bundle manifest,
/// i.e. "dna.yaml"
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, derive_more::Constructor)]
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

    fn path() -> PathBuf {
        "dna.yaml".into()
    }

    fn bundle_extension() -> &'static str {
        unimplemented!()
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
