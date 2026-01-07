//! App Manifest format version 0.
//!
//! **NB: do not modify the types in this file**!
//! (at least not after this initial schema has been stabilized).
//! For any modifications, create a new version of the spec and leave this one
//! alone to maintain backwards compatibility.
//!
//! This is the initial version of the App Manifest. Not all functionality is
//! implemented yet, notably:
//! - Using existing Cells is not implemented
//! - Specifying DNA version is not implemented (DNA migration needs to land first)

// Temporarily allowing deprecation because of [`CellProvisioning::UseExisting`].
#![allow(deprecated)]

use super::{
    app_manifest_validated::{AppManifestValidated, AppRoleManifestValidated},
    error::{AppManifestError, AppManifestResult},
};
use crate::prelude::{RoleName, YamlProperties};
use holo_hash::DnaHashB64;
use holochain_zome_types::prelude::*;
use schemars::JsonSchema;
use std::collections::HashMap;

/// Version 0 of the App manifest schema
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    JsonSchema,
    derive_builder::Builder,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct AppManifestV0 {
    /// Name of the App. This may be used as the installed_app_id.
    pub name: String,

    /// Description of the app, just for context.
    pub description: Option<String>,

    /// The roles that need to be filled (by DNAs) for this app.
    pub roles: Vec<AppRoleManifest>,

    /// Declares that the app may be installed without the need to
    /// specify membrane proofs at installation time. If memproofs are not
    /// provided at install time, they must be provided later before the
    /// app can be enabled. If memproofs are provided
    /// at install time, the app will be installed as normal, without the
    /// special deferred memproof flow.
    #[serde(default)]
    #[builder(default)]
    pub allow_deferred_memproofs: bool,

    /// URL of the bootstrap server to use for all Cells created
    /// for this app. If not provided here, the bootstrap server
    /// specified in the conductor config file will be used.
    #[serde(default)]
    #[builder(default)]
    pub bootstrap_url: Option<String>,

    /// URL of the signal server to use for all Cells created
    /// for this app. If not provided here, the signal server
    /// specified in the conductor config file will be used.
    #[serde(default)]
    #[builder(default)]
    pub signal_url: Option<String>,
}

/// Description of an app "role" defined by this app.
/// Roles get filled according to the provisioning rules, as well as by
/// potential runtime clones.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct AppRoleDnaManifest {
    /// Where to find this DNA.
    ///
    /// The DNA bundle at this path is included in the hApp bundle. The path is resolved relative
    /// to this app manifest file.
    pub path: Option<String>,

    /// Optional default modifier values.
    ///
    /// Overrides any default modifiers specified in the DNA file,
    /// and may also be overridden during installation.
    /// A set of modifiers completely overrides previously specified default properties,
    /// rather than being interpolated into them.
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

    /// Allow up to this many "clones" to be created at runtime.
    /// Default: 0
    #[serde(default)]
    pub clone_limit: u32,
}

impl AppRoleDnaManifest {
    /// Create a sample AppRoleDnaManifest as a template to be followed
    pub fn sample() -> Self {
        Self {
            path: Some("./path/to/my/dnabundle.dna".to_string()),
            modifiers: DnaModifiersOpt::none(),
            installed_hash: None,
            clone_limit: 0,
        }
    }
}

/// Rules to determine if and how a Cell will be created for this Dna
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[serde(tag = "strategy")]
#[allow(missing_docs)]
pub enum CellProvisioning {
    /// Always create a new Cell when installing this App
    Create { deferred: bool },

    #[deprecated(
        since = "0.6.0-dev.17",
        note = "For late binding, update the coordinators of a DNA. For calling cells of other apps, use bridge calls."
    )]
    /// Require that a Cell is already installed which has a DNA that's compatible with the
    /// `installed_hash` specified in the manifest.
    ///
    /// `protected` refers to the dependency. If the dependency is "protected", then the App
    /// which owns the Cell which is shared by this role cannot be uninstalled as long as
    /// this dependency exists. The dependency can be marked unprotected if this app is
    /// written such that it can still function with the dependency being unavailable.
    ///
    /// If the dependency is protected, the depended-upon App can still be uninstalled
    /// with the `AdminRequest::UninstallApp::force` flag
    UseExisting { protected: bool },

    /// Install or locate the DNA, but never create a Cell for this DNA.
    /// Only allow clones to be created from the DNA specified.
    /// This case requires `clone_limit > 0`, otherwise no Cells will ever be created.
    CloneOnly,
}

impl Default for CellProvisioning {
    fn default() -> Self {
        Self::Create { deferred: false }
    }
}

impl AppManifestV0 {
    /// Update the network seed for all DNAs used in Create-provisioned Cells.
    /// Cells with other provisioning strategies are not affected.
    pub fn set_network_seed(&mut self, network_seed: NetworkSeed) {
        for role in self.roles.iter_mut() {
            // Only update the network seed for roles for which it makes sense to do so
            match role.provisioning.clone().unwrap_or_default() {
                CellProvisioning::Create { .. } | CellProvisioning::CloneOnly => {
                    role.dna.modifiers.network_seed = Some(network_seed.clone());
                }
                _ => {}
            }
        }
    }

    /// Selectively overrides the modifiers for the given roles. Only fields with value `Some(T)` will
    /// override the corresponding value in the manifest. If `None` is provided for a modifier field
    /// the corresponding value in the manifest will remain untouched.
    pub fn override_modifiers(
        &mut self,
        modifiers: HashMap<RoleName, DnaModifiersOpt<YamlProperties>>,
    ) -> AppManifestResult<()> {
        let existing_role_names = self
            .roles
            .iter()
            .map(|manifest| &manifest.name)
            .collect::<Vec<&String>>();
        for role_name in modifiers.keys() {
            if !existing_role_names.contains(&role_name) {
                return Err(AppManifestError::InvalidRoleName(format!(
                    "Tried to set modifiers for a role name that does not exist in the app manifest: {role_name}"
                )));
            }
        }
        for role in self.roles.iter_mut() {
            if let Some(modifier_opts) = modifiers.get(&role.name) {
                if let Some(network_seed) = modifier_opts.network_seed.clone() {
                    role.dna.modifiers.network_seed = Some(network_seed);
                }
                if let Some(props) = modifier_opts.properties.clone() {
                    role.dna.modifiers.properties = Some(props);
                }
            }
        }
        Ok(())
    }

    /// Convert this human-focused manifest into a validated, concise representation
    pub fn validate(self) -> AppManifestResult<AppManifestValidated> {
        let AppManifestV0 {
            name,
            roles,
            description: _,
            allow_deferred_memproofs: _,
            bootstrap_url: _,
            signal_url: _,
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
                        path,
                        installed_hash,
                        clone_limit,
                        modifiers,
                    } = dna;
                    let modifiers = modifiers.serialized()?;
                    // Go from "flexible" enum into proper DnaVersionSpec.
                    let validated = match provisioning.unwrap_or_default() {
                        CellProvisioning::Create { deferred } => AppRoleManifestValidated::Create {
                            deferred,
                            clone_limit,
                            path: Self::require(path, "roles.dna.path")?,
                            modifiers,
                            installed_hash,
                        },
                        #[allow(deprecated)]
                        CellProvisioning::UseExisting { protected } => {
                            AppRoleManifestValidated::UseExisting {
                                protected,
                                compatible_hash: Self::require(
                                    installed_hash,
                                    "roles.dna.installed_hash",
                                )?,
                            }
                        }
                        CellProvisioning::CloneOnly => AppRoleManifestValidated::CloneOnly {
                            clone_limit,
                            path: Self::require(path, "roles.dna.path")?,
                            installed_hash,
                            modifiers,
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
    use holo_hash::fixt::*;

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
        file: Option<String>,
        installed_hash: DnaHash,
        modifiers: DnaModifiersOpt<YamlProperties>,
    ) -> AppManifestV0 {
        let roles = vec![AppRoleManifest {
            name: "role_name".into(),
            dna: AppRoleDnaManifest {
                path: file,
                modifiers,
                installed_hash: Some(installed_hash.into()),
                clone_limit: 50,
            },
            provisioning: Some(CellProvisioning::Create { deferred: false }),
        }];
        AppManifestV0 {
            name: "Test app".to_string(),
            description: Some("Serialization round trip test".to_string()),
            roles,
            allow_deferred_memproofs: false,
            bootstrap_url: Some("https://bootstrap.test".to_string()),
            signal_url: Some("wss://sbd.test".to_string()),
        }
    }

    #[tokio::test]
    async fn manifest_v0_roundtrip() {
        let file = Some("/tmp/test.dna".to_string());
        let modifiers = DnaModifiersOpt {
            properties: Some(app_manifest_properties_fixture()),
            network_seed: Some("network_seed".into()),
        };
        let installed_hash = fixt!(DnaHash);
        let manifest = app_manifest_fixture(file, installed_hash.clone(), modifiers).await;
        let manifest = AppManifest::from(manifest);
        let manifest_yaml = serde_yaml::to_string(&manifest).unwrap();
        let manifest_roundtrip = serde_yaml::from_str(&manifest_yaml).unwrap();

        assert_eq!(manifest, manifest_roundtrip);

        let expected_yaml = format!(
            r#"---

manifest_version: "0"
name: "Test app"
description: "Serialization roundtrip test"
roles:
  - name: "role_name"
    provisioning:
      strategy: "create"
      deferred: false
    dna:
      path: /tmp/test.dna
      installed_hash: {installed_hash}
      clone_limit: 50
      network_seed: network_seed
      modifiers:
        properties:
          salad: "bar"

        "#
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
    async fn manifest_v0_set_network_seed() {
        let mut manifest = AppManifestV0 {
            name: "test".to_string(),
            description: None,
            roles: vec![],
            allow_deferred_memproofs: false,
            bootstrap_url: None,
            signal_url: None,
        };
        manifest.roles = vec![
            AppRoleManifest {
                name: "test-role-1".to_string(),
                provisioning: None,
                dna: AppRoleDnaManifest {
                    path: None,
                    modifiers: DnaModifiersOpt::none(),
                    installed_hash: None,
                    clone_limit: 0,
                },
            },
            AppRoleManifest {
                name: "test-role-2".to_string(),
                provisioning: None,
                dna: AppRoleDnaManifest {
                    path: None,
                    modifiers: DnaModifiersOpt::none(),
                    installed_hash: None,
                    clone_limit: 0,
                },
            },
        ];
        manifest.roles[0].provisioning = Some(CellProvisioning::Create { deferred: false });
        manifest.roles[1].provisioning = Some(CellProvisioning::Create { deferred: false });

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
    }
}
