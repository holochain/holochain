use crate::prelude::*;
use mr_bundle::ResourceIdentifier;
use schemars::JsonSchema;
use std::collections::HashMap;
use std::collections::HashSet;

mod dna_manifest_v0;

#[cfg(test)]
mod test;

/// Re-export the current version. When creating a new version, just re-export
/// the new version, and update the code accordingly.
pub use dna_manifest_v0::{
    DnaManifestV0 as DnaManifestCurrent, DnaManifestV0Builder as DnaManifestCurrentBuilder, *,
};

/// The enum which encompasses all versions of the DNA manifest, past and present.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema, derive_more::From,
)]
#[serde(tag = "manifest_version")]
#[allow(missing_docs)]
pub enum DnaManifest {
    #[serde(rename = "0")]
    V0(DnaManifestV0),
}

#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, shrinkwraprs::Shrinkwrap,
)]
#[serde(try_from = "DnaManifest")]
/// A dna manifest that has been successfully validated.
pub struct ValidatedDnaManifest(pub DnaManifest);

impl mr_bundle::Manifest for ValidatedDnaManifest {
    fn generate_resource_ids(&mut self) -> HashMap<ResourceIdentifier, String> {
        match &mut self.0 {
            DnaManifest::V0(m) => m
                .all_zomes_mut()
                .map(|zome| {
                    let id = zome.resource_id();
                    let file = zome.path.clone();

                    zome.path = id.clone();

                    (id, file)
                })
                .collect(),
        }
    }

    fn resource_ids(&self) -> Vec<ResourceIdentifier> {
        match &self.0 {
            DnaManifest::V0(m) => m.all_zomes().map(|zome| zome.resource_id()).collect(),
        }
    }

    fn file_name() -> &'static str {
        "dna.yaml"
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
        network_seed: Option<String>,
        properties: Option<YamlProperties>,
        integrity_zomes: Vec<ZomeManifest>,
        coordinator_zomes: Vec<ZomeManifest>,
        #[cfg(feature = "unstable-migration")] lineage: Vec<DnaHash>,
    ) -> Self {
        DnaManifestCurrent::new(
            name,
            IntegrityManifest::new(network_seed, properties, integrity_zomes),
            CoordinatorManifest {
                zomes: coordinator_zomes,
            },
            #[cfg(feature = "unstable-migration")]
            lineage.into_iter().map(Into::into).collect(),
        )
        .into()
    }

    /// Getter for properties
    pub fn properties(&self) -> Option<YamlProperties> {
        match self {
            DnaManifest::V0(manifest) => manifest.integrity.properties.clone(),
        }
    }

    /// Getter for network_seed
    pub fn network_seed(&self) -> Option<String> {
        match self {
            DnaManifest::V0(manifest) => manifest.integrity.network_seed.clone(),
        }
    }

    /// Getter for name
    pub fn name(&self) -> String {
        match self {
            DnaManifest::V0(manifest) => manifest.name.clone(),
        }
    }
}

impl TryFrom<DnaManifest> for ValidatedDnaManifest {
    type Error = DnaError;

    fn try_from(value: DnaManifest) -> Result<Self, Self::Error> {
        match &value {
            DnaManifest::V0(m) => {
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

impl TryFrom<DnaManifestV0> for ValidatedDnaManifest {
    type Error = DnaError;

    fn try_from(value: DnaManifestV0) -> Result<Self, Self::Error> {
        DnaManifest::from(value).try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults() {
        let manifest: DnaManifest = DnaManifestCurrentBuilder::default()
            .name("my_dna".to_owned())
            .integrity(IntegrityManifest {
                network_seed: None,
                properties: None,
                zomes: vec![],
            })
            .build()
            .unwrap()
            .into();

        match &manifest {
            DnaManifest::V0(m) => {
                assert_eq!(m.coordinator, CoordinatorManifest::default());
                #[cfg(feature = "unstable-migration")]
                assert_eq!(m.lineage, vec![]);
            }
        }

        let s = serde_yaml::to_string(&manifest).unwrap();
        println!("{s}");
    }
}
