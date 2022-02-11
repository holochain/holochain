use crate::prelude::*;
use std::path::PathBuf;
mod dna_manifest_v1;

/// Re-export the current version. When creating a new version, just re-export
/// the new version, and update the code accordingly.
pub use dna_manifest_v1::{
    DnaManifestV1 as DnaManifestCurrent, DnaManifestV1Builder as DnaManifestCurrentBuilder, *,
};

/// The enum which encompasses all versions of the DNA manifest, past and present.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
#[serde(tag = "manifest_version")]
#[allow(missing_docs)]
pub enum DnaManifest {
    #[serde(rename = "1")]
    V1(DnaManifestV1),
}

impl mr_bundle::Manifest for DnaManifest {
    fn locations(&self) -> Vec<mr_bundle::Location> {
        match self {
            Self::V1(m) => m.zomes.iter().map(|zome| zome.location.clone()).collect(),
        }
    }

    fn path() -> PathBuf {
        "dna.yaml".into()
    }

    fn bundle_extension() -> &'static str {
        "dna"
    }
}

impl DnaManifest {
    /// Create a DnaManifest based on the current version.
    /// Be sure to update this function when creating a new version.
    pub fn current(
        name: String,
        uid: Option<String>,
        properties: Option<YamlProperties>,
        origin_time: HumanTimestamp,
        zomes: Vec<ZomeManifest>,
    ) -> Self {
        DnaManifestCurrent::new(name, uid, properties, origin_time, zomes).into()
    }

    /// Getter for properties
    pub fn properties(&self) -> Option<YamlProperties> {
        match self {
            DnaManifest::V1(manifest) => manifest.properties.clone(),
        }
    }

    /// Getter for uid
    pub fn uid(&self) -> Option<String> {
        match self {
            DnaManifest::V1(manifest) => manifest.uid.clone(),
        }
    }

    /// Getter for name
    pub fn name(&self) -> String {
        match self {
            DnaManifest::V1(manifest) => manifest.name.clone(),
        }
    }
}
