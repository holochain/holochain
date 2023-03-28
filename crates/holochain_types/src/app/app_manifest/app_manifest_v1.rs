//! App Manifest format, installed_hash 1.
//!
//! NB: After stabilization, *do not modify this file*! Create a new installed_hash of
//! the spec and leave this one alone to maintain backwards compatibility.

use super::{
    app_manifest_validated::{AppManifestValidated, AppRoleManifestValidated},
    error::{AppManifestError, AppManifestResult},
};
use crate::prelude::{RoleName, YamlProperties};
use holo_hash::DnaHashB64;
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
    pub name: RoleName,

    /// Determines if, how, and when a Cell will be provisioned.
    pub provisioning: Option<CellProvisioning>,

    /// Declares where to find the DNA, and options to modify it before
    /// inclusion in a Cell
    pub dna: AppRoleDnaManifest,
}

impl AppRoleManifest {
    /// Create a sample AppRoleManifest as a template to be followed
    pub fn sample(name: RoleName) -> Self {
        Self {
            name,
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

    /// The hash of the DNA to be installed. If specified, will cause installation to
    /// fail if the bundled DNA hash does not match this.
    ///
    /// Also allows the conductor to search for an already-installed DNA using this hash,
    /// which allows for re-installing an app which has already been installed by manifest
    /// only (no need to include the DNAs, since they are already installed in the conductor).
    /// In this case, `location` does not even need to be set.
    #[serde(default)]
    pub installed_hash: Option<DnaHashB64>,

    /// For backward compatibility only: `installed_hash` used to take a list of hashes.
    /// To prevent breaking manifests, this is still allowed, but now has no effect.
    #[serde(default)]
    #[serde(alias = "version")]
    pub _version: Option<VecOrSingle<DnaHashB64>>,

    /// Allow up to this many "clones" to be created at runtime.
    /// Each runtime clone is created by the `CreateClone` strategy,
    /// regardless of the provisioning strategy set in the manifest.
    /// Default: 0
    #[serde(default)]
    pub clone_limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[serde(untagged)]
/// A vec of values or a single value.
pub enum VecOrSingle<T> {
    /// A vec of values
    Vec(Vec<T>),
    /// A single value
    Single(T),
}

impl AppRoleDnaManifest {
    /// Create a sample AppRoleDnaManifest as a template to be followed
    pub fn sample() -> Self {
        Self {
            location: Some(mr_bundle::Location::Bundled(
                "./path/to/my/dnabundle.dna".into(),
            )),
            modifiers: DnaModifiersOpt::none(),
            installed_hash: None,
            _version: None,
            clone_limit: 0,
        }
    }
}

/// Specifies remote, local, or bundled location of DNA
pub type DnaLocation = mr_bundle::Location;

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
    /// Require that a Cell is already installed which matches the DNA installed_hash
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
            if !matches!(
                role.provisioning.clone().unwrap_or_default(),
                CellProvisioning::CreateClone { .. } | CellProvisioning::UseExisting { .. }
            ) {
                // Only update the network seed for roles for which it makes sense to do so
                role.dna.modifiers.network_seed = Some(network_seed.clone());
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
                     name,
                     provisioning,
                     dna,
                 }| {
                    let AppRoleDnaManifest {
                        location,
                        installed_hash,
                        clone_limit,
                        modifiers,
                        _version: _,
                    } = dna;
                    let modifiers = modifiers.serialized()?;
                    // Go from "flexible" enum into proper DnaVersionSpec.
                    let installed_hash = installed_hash.map(Into::into);
                    let validated = match provisioning.unwrap_or_default() {
                        CellProvisioning::Create { deferred } => AppRoleManifestValidated::Create {
                            deferred,
                            clone_limit,
                            location: Self::require(location, "roles.dna.(path|url)")?,
                            modifiers,
                            installed_hash,
                        },
                        CellProvisioning::CreateClone { deferred } => {
                            AppRoleManifestValidated::CreateClone {
                                deferred,
                                clone_limit,
                                location: Self::require(location, "roles.dna.(path|url)")?,
                                modifiers,
                                installed_hash,
                            }
                        }
                        CellProvisioning::UseExisting { deferred } => {
                            AppRoleManifestValidated::UseExisting {
                                deferred,
                                clone_limit,
                                installed_hash: Self::require(
                                    installed_hash,
                                    "roles.dna.installed_hash",
                                )?,
                            }
                        }
                        CellProvisioning::CreateIfNotExists { deferred } => {
                            AppRoleManifestValidated::CreateIfNotExists {
                                deferred,
                                clone_limit,
                                location: Self::require(location, "roles.dna.(path|url)")?,
                                installed_hash: Self::require(
                                    installed_hash,
                                    "roles.dna.installed_hash",
                                )?,
                                modifiers,
                            }
                        }
                        CellProvisioning::Disabled => AppRoleManifestValidated::Disabled {
                            clone_limit,
                            installed_hash: Self::require(
                                installed_hash,
                                "roles.dna.installed_hash",
                            )?,
                        },
                    };
                    AppManifestResult::Ok((name, validated))
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

    pub async fn app_manifest_fixture(
        location: Option<mr_bundle::Location>,
        installed_hash: DnaHash,
        modifiers: DnaModifiersOpt<YamlProperties>,
    ) -> AppManifestV1 {
        let roles = vec![AppRoleManifest {
            name: "role_name".into(),
            dna: AppRoleDnaManifest {
                location,
                modifiers,
                installed_hash: Some(installed_hash.into()),
                clone_limit: 50,
                _version: None,
            },
            provisioning: Some(CellProvisioning::Create { deferred: false }),
        }];
        AppManifestV1 {
            name: "Test app".to_string(),
            description: Some("Serialization roundtrip test".to_string()),
            roles,
        }
    }

    #[tokio::test]
    async fn manifest_v1_roundtrip() {
        let location = Some(mr_bundle::Location::Path(PathBuf::from("/tmp/test.dna")));
        let modifiers = DnaModifiersOpt {
            properties: Some(app_manifest_properties_fixture()),
            network_seed: Some("network_seed".into()),
            origin_time: None,
            quantum_time: None,
        };
        let installed_hash = fixt!(DnaHash);
        let manifest = app_manifest_fixture(location, installed_hash.clone(), modifiers).await;
        let manifest = AppManifest::from(manifest);
        let manifest_yaml = serde_yaml::to_string(&manifest).unwrap();
        let manifest_roundtrip = serde_yaml::from_str(&manifest_yaml).unwrap();

        assert_eq!(manifest, manifest_roundtrip);

        let expected_yaml = format!(
            r#"---

manifest_version: "1"
name: "Test app"
description: "Serialization roundtrip test"
roles:
  - name: "role_name"
    provisioning:
      strategy: "create"
      deferred: false
    dna:
      path: /tmp/test.dna
      installed_hash: {}
      version:
        - {}
        - {}
      clone_limit: 50
      network_seed: network_seed
      modifiers:
        properties:
          salad: "bar"

        "#,
            installed_hash, installed_hash, installed_hash,
        );
        let actual = serde_yaml::to_value(&manifest).unwrap();
        let expected: serde_yaml::Value = serde_yaml::from_str(&expected_yaml).unwrap();

        // Check a handful of fields. Order matters in YAML, so to check the
        // entire structure would be too fragile for testing.

        for getter in [
            |v: &serde_yaml::Value| v["roles"][0]["name"].clone(),
            |v: &serde_yaml::Value| v["roles"][0]["provisioning"]["deferred"].clone(),
            |v: &serde_yaml::Value| v["roles"][0]["dna"]["installed_hash"].clone(),
            |v: &serde_yaml::Value| v["roles"][0]["dna"]["modifiers"]["properties"].clone(),
        ] {
            let left = getter(&actual);
            let right = getter(&expected);
            assert_eq!(left, right);
            assert!(!left.is_null());
        }
    }

    #[tokio::test]
    async fn manifest_v1_roundtrip_backward_compat_installed_hash() {
        let hash1 = fixt!(DnaHashB64);
        let hash2 = fixt!(DnaHashB64);
        let yaml1 = format!(
            r#"---

manifest_version: "1"
name: "Test app"
description: "Serialization works using a single value in `version`"
roles:
  - name: "role_name"
    provisioning:
      strategy: "create"
      deferred: false
    dna:
      path: /tmp/test.dna
      version: {}

        "#,
            hash1
        );
        let yaml2 = format!(
            r#"---

manifest_version: "1"
name: "Test app"
description: "Serialization works using multiple values in `version`"
roles:
  - name: "role_name"
    dna:
      path: /tmp/test.dna
      version:
        - {}
        - {}
        
        "#,
            hash1, hash2
        );

        // dbg!(serde_yaml::from_str::<serde_yaml::Value>(&yaml1).unwrap());
        // dbg!(serde_yaml::from_str::<serde_yaml::Value>(&yaml2).unwrap());
        let manifest1: AppManifest = serde_yaml::from_str(&yaml1).unwrap();
        let manifest2: AppManifest = serde_yaml::from_str(&yaml2).unwrap();

        {
            let AppManifest::V1(AppManifestCurrent { roles, .. }) = manifest1;
            assert_eq!(
                roles[0].dna._version,
                Some(VecOrSingle::Single(hash1.clone()))
            );
            assert!(roles[0].dna.installed_hash.is_none());
        }
        {
            let AppManifest::V1(AppManifestCurrent { roles, .. }) = manifest2;
            assert_eq!(
                roles[0].dna._version,
                Some(VecOrSingle::Vec(vec![hash1, hash2]))
            );
            assert!(roles[0].dna.installed_hash.is_none());
        }
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
