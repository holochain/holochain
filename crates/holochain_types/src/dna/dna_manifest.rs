use crate::prelude::*;
use std::{collections::HashSet, path::PathBuf};
mod dna_manifest_v1;

#[cfg(test)]
mod test;

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

#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, shrinkwraprs::Shrinkwrap,
)]
#[serde(try_from = "DnaManifest")]
/// A dna manifest that has been successfully validated.
pub struct ValidatedDnaManifest(pub(super) DnaManifest);

impl mr_bundle::Manifest for ValidatedDnaManifest {
    fn locations(&self) -> Vec<mr_bundle::Location> {
        match &self.0 {
            DnaManifest::V1(m) => m.all_zomes().map(|zome| zome.location.clone()).collect(),
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
        integrity_zomes: Vec<ZomeManifest>,
        coordinator_zomes: Vec<ZomeManifest>,
    ) -> Self {
        DnaManifestCurrent::new(
            name,
            IntegrityManifest::new(uid, properties, origin_time, integrity_zomes),
            CoordinatorManifest {
                zomes: coordinator_zomes,
            },
        )
        .into()
    }

    /// Getter for properties
    pub fn properties(&self) -> Option<YamlProperties> {
        match self {
            DnaManifest::V1(manifest) => manifest.integrity.properties.clone(),
        }
    }

    /// Getter for uid
    pub fn uid(&self) -> Option<String> {
        match self {
            DnaManifest::V1(manifest) => manifest.integrity.uid.clone(),
        }
    }

    /// Getter for name
    pub fn name(&self) -> String {
        match self {
            DnaManifest::V1(manifest) => manifest.name.clone(),
        }
    }
}

impl TryFrom<DnaManifest> for ValidatedDnaManifest {
    type Error = DnaError;

    fn try_from(value: DnaManifest) -> Result<Self, Self::Error> {
        match &value {
            DnaManifest::V1(m) => {
                let integrity_zome_names: HashSet<_> =
                    m.integrity.zomes.iter().map(|z| z.name.clone()).collect();
                // Check there are no duplicate zome names.
                let mut names = HashSet::new();
                for z in m.all_zomes() {
                    if !names.insert(z.name.clone()) {
                        return Err(DnaError::DuplicateZomeNames(z.name.to_string()));
                    }
                    if let Some(dependencies) = &z.dependencies {
                        // Check the dependency zome names exist in the integrity zomes
                        // and does not point to self.
                        if let Some(dep) = dependencies.iter().find(|ZomeDependency { name }| {
                            !integrity_zome_names.contains(name) || *name == z.name
                        }) {
                            return Err(DnaError::DanglingZomeDependency(
                                dep.name.to_string(),
                                z.name.to_string(),
                            ));
                        }
                    }
                }
            }
        }
        Ok(Self(value))
    }
}

impl TryFrom<DnaManifestV1> for ValidatedDnaManifest {
    type Error = DnaError;

    fn try_from(value: DnaManifestV1) -> Result<Self, Self::Error> {
        DnaManifest::from(value).try_into()
    }
}
