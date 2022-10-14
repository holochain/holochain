//! App Manifest format, version 1.
//!
//! NB: After stabilization, *do not modify this file*! Create a new version of
//! the spec and leave this one alone to maintain backwards compatibility.

use super::{
    app_manifest_validated::{AppManifestValidated, AppRoleManifestValidated},
    error::{AppManifestError, AppManifestResult},
};
use crate::prelude::{AppRoleId, YamlProperties};
use holo_hash::{DnaHash, DnaHashB64};
use holochain_zome_types::{DnaModifiersOpt, NetworkSeed};
use std::collections::HashMap;

/// Version 1 of the App manifest schema
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_builder::Builder,
)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AppManifestV1 {
    /// Name of the App. This may be used as the installed_app_id.
    pub name: String,

    /// Description of the app, just for context.
    pub description: Option<String>,

    /// The roles that need to be filled (by DNAs) for this app.
    pub roles: Vec<AppRoleManifest>,
}

/// Description of an app "role" defined by this app.
/// Roles get filled according to the provisioning rules, as well as by
/// potential runtime clones.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AppRoleManifest {
    /// The ID which will be used to refer to:
    /// - this role,
    /// - the DNA which fills it,
    /// - and the cell(s) created from that DNA
    pub id: AppRoleId,

    /// Determines if, how, and when a Cell will be provisioned.
    pub provisioning: Option<CellProvisioning>,

    /// Declares where to find the DNA, and options to modify it before
    /// inclusion in a Cell
    pub dna: AppRoleDnaManifest,
}

impl AppRoleManifest {
    /// Create a sample AppRoleManifest as a template to be followed
    pub fn sample(id: AppRoleId) -> Self {
        Self {
            id,
            provisioning: Some(CellProvisioning::default()),
            dna: AppRoleDnaManifest::sample(),
        }
    }
}

/// The DNA portion of an app role
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AppRoleDnaManifest {
    /// Where to find this Dna. To specify a DNA included in a hApp Bundle,
    /// use a local relative path that corresponds with the bundle structure.
    ///
    /// Note that since this is flattened,
    /// there is no actual "location" key in the manifest.
    #[serde(flatten)]
    pub location: Option<mr_bundle::Location>,

    /// Optional default modifier values. May be overridden during installation.
    #[serde(default)]
    pub modifiers: DnaModifiersOpt<YamlProperties>,

    /// The versioning constraints for the DNA. Ensures that only a DNA that
    /// matches the version spec will be used.
    pub version: Option<DnaVersionFlexible>,

    /// Allow up to this many "clones" to be created at runtime.
    /// Each runtime clone is created by the `CreateClone` strategy,
    /// regardless of the provisioning strategy set in the manifest.
    /// Default: 0
    #[serde(default)]
    pub clone_limit: u32,
}

impl AppRoleDnaManifest {
    /// Create a sample AppRoleDnaManifest as a template to be followed
    pub fn sample() -> Self {
        Self {
            location: Some(mr_bundle::Location::Bundled(
                "./path/to/my/dnabundle.dna".into(),
            )),
            modifiers: DnaModifiersOpt::none(),
            version: None,
            clone_limit: 0,
        }
    }
}

/// Allow the DNA version to be specified as a single hash, rather than a
/// singleton list. Just a convenience.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum DnaVersionFlexible {
    /// A version spec with a single hash
    Singleton(DnaHashB64),
    /// An actual version spec
    Multiple(DnaVersionSpec),
}

impl From<DnaVersionFlexible> for DnaVersionSpec {
    fn from(v: DnaVersionFlexible) -> Self {
        match v {
            DnaVersionFlexible::Singleton(h) => DnaVersionSpec(vec![h]),
            DnaVersionFlexible::Multiple(v) => v,
        }
    }
}

/// Specifies remote, local, or bundled location of DNA
pub type DnaLocation = mr_bundle::Location;

/// Defines a criterion for a DNA version to match against.
///
/// Currently we're using the most simple possible version spec: A list of
/// valid DnaHashes. The order of the list is from latest version to earliest.
/// In subsequent manifest versions, this will become more expressive.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct DnaVersionSpec(Vec<DnaHashB64>);

// NB: the following is likely to remain in the API for DnaVersionSpec
impl DnaVersionSpec {
    /// Check if a DNA satisfies this version spec
    pub fn matches(&self, hash: DnaHash) -> bool {
        self.0.contains(&hash.into())
    }
}

// NB: the following is likely to be removed from the API for DnaVersionSpec
// after our versioning becomes more sophisticated
impl DnaVersionSpec {
    /// Return the list of hashes covered by a version (obviously temporary,
    /// while we don't have real versioning)
    pub fn dna_hashes(&self) -> Vec<&DnaHashB64> {
        self.0.iter().collect()
    }
}

/// Rules to determine if and how a Cell will be created for this Dna
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "strategy")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[allow(missing_docs)]
pub enum CellProvisioning {
    /// Always create a new Cell when installing this App
    Create { deferred: bool },
    /// Always create a new Cell when installing the App,
    /// and use a unique network seed to ensure a distinct DHT network
    CreateClone { deferred: bool },
    /// Require that a Cell is already installed which matches the DNA version
    /// spec, and which has an Agent that's associated with this App's agent
    /// via DPKI. If no such Cell exists, *app installation fails*.
    UseExisting { deferred: bool },
    /// Try `UseExisting`, and if that fails, fallback to `Create`
    CreateIfNotExists { deferred: bool },
    /// Disallow provisioning altogether. In this case, we expect
    /// `clone_limit > 0`: otherwise, no Cells will ever be created.
    Disabled,
}

impl Default for CellProvisioning {
    fn default() -> Self {
        Self::Create { deferred: false }
    }
}

impl AppManifestV1 {
    /// Update the network seed for all DNAs used in Create-provisioned Cells.
    /// Cells with other provisioning strategies are not affected.
    ///
    // TODO: it probably makes sense to do this for CreateIfNotExists cells
    // too, in the Create case, but we would have to do that during installation
    // rather than simply updating the manifest. Let's hold off on that until
    // we know we need it, since this way is substantially simpler.
    pub fn set_network_seed(&mut self, network_seed: NetworkSeed) {
        for mut role in self.roles.iter_mut() {
            if matches!(role.provisioning, Some(CellProvisioning::Create { .. })) {
                role.dna.modifiers =
                    DnaModifiersOpt::none().with_network_seed(network_seed.clone());
            }
        }
    }

    /// Convert this human-focused manifest into a validated, concise representation
    pub fn validate(self) -> AppManifestResult<AppManifestValidated> {
        let AppManifestV1 {
            name,
            roles,
            description: _,
        } = self;
        let roles = roles
            .into_iter()
            .map(
                |AppRoleManifest {
                     id,
                     provisioning,
                     dna,
                 }| {
                    let AppRoleDnaManifest {
                        location,
                        version,
                        clone_limit,
                        modifiers,
                    } = dna;
                    let modifiers = modifiers.serialized()?;
                    // Go from "flexible" enum into proper DnaVersionSpec.
                    let version = version.map(Into::into);
                    let validated = match provisioning.unwrap_or_default() {
                        CellProvisioning::Create { deferred } => AppRoleManifestValidated::Create {
                            deferred,
                            clone_limit,
                            location: Self::require(location, "roles.dna.(path|url)")?,
                            modifiers,
                            version,
                        },
                        CellProvisioning::CreateClone { deferred } => {
                            AppRoleManifestValidated::CreateClone {
                                deferred,
                                clone_limit,
                                location: Self::require(location, "roles.dna.(path|url)")?,
                                modifiers,
                                version,
                            }
                        }
                        CellProvisioning::UseExisting { deferred } => {
                            AppRoleManifestValidated::UseExisting {
                                deferred,
                                clone_limit,
                                version: Self::require(version, "roles.dna.version")?,
                            }
                        }
                        CellProvisioning::CreateIfNotExists { deferred } => {
                            AppRoleManifestValidated::CreateIfNotExists {
                                deferred,
                                clone_limit,
                                location: Self::require(location, "roles.dna.(path|url)")?,
                                version: Self::require(version, "roles.dna.version")?,
                                modifiers,
                            }
                        }
                        CellProvisioning::Disabled => AppRoleManifestValidated::Disabled {
                            clone_limit,
                            version: Self::require(version, "roles.dna.version")?,
                        },
                    };
                    AppManifestResult::Ok((id, validated))
                },
            )
            .collect::<Result<HashMap<_, _>, _>>()?;
        AppManifestValidated::new(name, roles)
    }

    fn require<T>(maybe: Option<T>, context: &str) -> AppManifestResult<T> {
        maybe.ok_or_else(|| AppManifestError::MissingField(context.to_owned()))
    }
}

#[cfg(test)]
pub mod tests {
    use futures::future::join_all;

    use super::*;
    use crate::app::app_manifest::AppManifest;
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use std::path::PathBuf;

    #[cfg(feature = "arbitrary")]
    use arbitrary::Arbitrary;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Props {
        salad: String,
    }

    pub fn app_manifest_properties_fixture() -> YamlProperties {
        YamlProperties::new(
            serde_yaml::to_value(Props {
                salad: "bar".to_string(),
            })
            .unwrap(),
        )
    }

    pub async fn app_manifest_fixture<I: IntoIterator<Item = DnaDef>>(
        location: Option<mr_bundle::Location>,
        dnas: I,
    ) -> (AppManifest, Vec<DnaHashB64>) {
        let hashes = join_all(
            dnas.into_iter()
                .map(|dna| async move { DnaHash::with_data_sync(&dna).into() }),
        )
        .await;

        let version = DnaVersionSpec::from(hashes.clone()).into();
        let modifiers = DnaModifiersOpt {
            properties: Some(app_manifest_properties_fixture()),
            network_seed: Some("network_seed".into()),
            origin_time: None,
        };

        let roles = vec![AppRoleManifest {
            id: "role_id".into(),
            dna: AppRoleDnaManifest {
                location,
                modifiers,
                version: Some(version),
                clone_limit: 50,
            },
            provisioning: Some(CellProvisioning::Create { deferred: false }),
        }];
        let manifest = AppManifest::V1(AppManifestV1 {
            name: "Test app".to_string(),
            description: Some("Serialization roundtrip test".to_string()),
            roles,
        });
        (manifest, hashes)
    }

    #[tokio::test]
    async fn manifest_v1_roundtrip() {
        let location = Some(mr_bundle::Location::Path(PathBuf::from("/tmp/test.dna")));
        let (manifest, dna_hashes) =
            app_manifest_fixture(location, vec![fixt!(DnaDef), fixt!(DnaDef)]).await;
        let manifest_yaml = serde_yaml::to_string(&manifest).unwrap();
        let manifest_roundtrip = serde_yaml::from_str(&manifest_yaml).unwrap();

        assert_eq!(manifest, manifest_roundtrip);

        let expected_yaml = format!(
            r#"---

manifest_version: "1"
name: "Test app"
description: "Serialization roundtrip test"
roles:
  - id: "role_id"
    provisioning:
      strategy: "create"
      deferred: false
    dna:
      path: /tmp/test.dna
      version:
        - {}
        - {}
      clone_limit: 50
      network_seed: network_seed
      properties:
        salad: "bar"

        "#,
            dna_hashes[0], dna_hashes[1]
        );
        let actual = serde_yaml::to_value(&manifest).unwrap();
        let expected: serde_yaml::Value = serde_yaml::from_str(&expected_yaml).unwrap();

        // Check a handful of fields. Order matters in YAML, so to check the
        // entire structure would be too fragile for testing.
        let fields = &[
            "roles[0].id",
            "roles[0].provisioning.deferred",
            "roles[0].dna.version[1]",
            "roles[0].dna.properties",
        ];
        assert_eq!(actual.get(fields[0]), expected.get(fields[0]));
        assert_eq!(actual.get(fields[1]), expected.get(fields[1]));
        assert_eq!(actual.get(fields[2]), expected.get(fields[2]));
        assert_eq!(actual.get(fields[3]), expected.get(fields[3]));
    }

    #[tokio::test]
    async fn manifest_v1_set_network_seed() {
        let mut u = arbitrary::Unstructured::new(&[0]);
        let mut manifest = AppManifestV1::arbitrary(&mut u).unwrap();
        manifest.roles = vec![
            AppRoleManifest::arbitrary(&mut u).unwrap(),
            AppRoleManifest::arbitrary(&mut u).unwrap(),
            AppRoleManifest::arbitrary(&mut u).unwrap(),
            AppRoleManifest::arbitrary(&mut u).unwrap(),
        ];
        manifest.roles[0].provisioning = Some(CellProvisioning::Create { deferred: false });
        manifest.roles[1].provisioning = Some(CellProvisioning::Create { deferred: false });
        manifest.roles[2].provisioning = Some(CellProvisioning::UseExisting { deferred: false });
        manifest.roles[3].provisioning =
            Some(CellProvisioning::CreateIfNotExists { deferred: false });

        let network_seed = NetworkSeed::from("blabla");
        manifest.set_network_seed(network_seed.clone());

        // - The Create roles have the network seed rewritten.
        assert_eq!(
            manifest.roles[0].dna.modifiers.network_seed.as_ref(),
            Some(&network_seed)
        );
        assert_eq!(
            manifest.roles[1].dna.modifiers.network_seed.as_ref(),
            Some(&network_seed)
        );

        // - The others do not.
        assert_ne!(
            manifest.roles[2].dna.modifiers.network_seed.as_ref(),
            Some(&network_seed)
        );
        assert_ne!(
            manifest.roles[3].dna.modifiers.network_seed.as_ref(),
            Some(&network_seed)
        );
    }
}
