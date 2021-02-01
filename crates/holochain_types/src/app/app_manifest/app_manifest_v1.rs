use super::{
    app_manifest_validated::{AppManifestValidated, CellManifestValidated},
    error::{AppManifestError, AppManifestResult},
};
use crate::prelude::{CellNick, YamlProperties};
use holo_hash::{DnaHash, DnaHashB64};
use std::{collections::HashMap, path::PathBuf};

pub type Uuid = String;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AppManifestV1 {
    /// Name of the App. This may be used as the installed_app_id.
    name: String,

    /// Description of the app, just for context.
    description: String,

    /// The Cell manifests that make up this app.
    cells: Vec<CellManifest>,
}

/// Description of a new or existing Cell referenced by this Bundle
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CellManifest {
    /// The CellNick which will be given to the installed Cell for this Dna.
    nick: CellNick,

    /// Determines if, how, and when a Cell will be provisioned.
    provisioning: Option<CellProvisioning>,

    /// Declares where to find the DNA, and options to modify it before
    /// inclusion in a Cell
    dna: DnaManifest,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DnaManifest {
    /// Where to find this Dna. To specify a DNA included in a hApp Bundle,
    /// use a local relative path that corresponds with the bundle structure.
    ///
    /// Note that since this is flattened,
    /// there is no actual "location" key in the manifest.
    #[serde(flatten)]
    location: Option<DnaLocation>,

    /// Optional default properties. May be overridden during installation.
    properties: Option<YamlProperties>,

    /// Optional fixed UUID. May be overridden during installation.
    uuid: Option<Uuid>,

    /// The versioning constraints for the DNA. Ensures that only a DNA that
    /// matches the version spec will be used.
    version: Option<DnaVersionSpec>,

    /// Allow up to this many "clones" to be created at runtime.
    /// Each runtime clone is created by the `CreateClone` strategy,
    /// regardless of the provisioning strategy set in the manifest.
    /// Default: 0
    #[serde(default)]
    clone_limit: u32,
}

/// Where to find this Dna.
/// If Local, the path may refer to a Dna which is bundled with the manifest,
/// or it may be to some other absolute or relative file path.
///
/// This representation, with named fields, is chosen so that in the yaml config,
/// either "path" or "url" can be specified due to this field being flattened.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum DnaLocation {
    /// Get Dna from local filesystem
    Local { path: PathBuf },

    /// Get Dna from URL
    Remote { url: String },
}

/// Defines a criterion for a DNA version to match against.
///
/// Currently we're using the most simple possible version spec: A list of
/// valid DnaHashes. The order of the list is from earliest to latest version.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
pub struct DnaVersionSpec(Vec<DnaHashB64>);

impl DnaVersionSpec {
    /// Check if a DNA satisfies this version spec
    pub fn _matches(&self, hash: DnaHash) -> bool {
        self.0.contains(&hash.into())
    }
}

/// Rules to determine if and how a Cell will be created for this Dna
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "strategy")]
pub enum CellProvisioning {
    /// Always create a new Cell when installing this App
    Create { deferred: bool },
    /// Always create a new Cell when installing the App,
    /// and use a unique UUID to ensure a distinct DHT network
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
    pub fn validate(self) -> AppManifestResult<AppManifestValidated> {
        let AppManifestV1 {
            name,
            cells,
            description: _,
        } = self;
        let cells = cells
            .into_iter()
            .map(
                |CellManifest {
                     nick,
                     provisioning,
                     dna,
                 }| {
                    let DnaManifest {
                        location,
                        properties,
                        version,
                        uuid,
                        clone_limit,
                    } = dna;
                    let validated = match provisioning.unwrap_or_default() {
                        CellProvisioning::Create { deferred } => CellManifestValidated::Create {
                            deferred,
                            clone_limit,
                            location: Self::require(location, "cells.dna.(path|url)")?,
                            properties,
                            uuid,
                            version,
                        },
                        CellProvisioning::CreateClone { deferred } => {
                            CellManifestValidated::CreateClone {
                                deferred,
                                clone_limit,
                                location: Self::require(location, "cells.dna.(path|url)")?,
                                properties,
                                version,
                            }
                        }
                        CellProvisioning::UseExisting { deferred } => {
                            CellManifestValidated::UseExisting {
                                deferred,
                                clone_limit,
                                version: Self::require(version, "cells.dna.version")?,
                            }
                        }
                        CellProvisioning::CreateIfNotExists { deferred } => {
                            CellManifestValidated::CreateIfNotExists {
                                deferred,
                                clone_limit,
                                location: Self::require(location, "cells.dna.(path|url)")?,
                                version: Self::require(version, "cells.dna.version")?,
                                properties,
                                uuid,
                            }
                        }
                        CellProvisioning::Disabled => {
                            CellManifestValidated::Disabled { clone_limit }
                        }
                    };
                    Ok((nick, validated))
                },
            )
            .collect::<Result<HashMap<_, _>, _>>()?;
        AppManifestValidated::new(name, cells)
    }

    fn require<T>(maybe: Option<T>, context: &str) -> AppManifestResult<T> {
        maybe.ok_or_else(|| AppManifestError::MissingField(context.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::app_manifest::AppManifest;
    use crate::prelude::YamlProperties;
    use std::path::PathBuf;

    #[test]
    fn manifest_v1_roundtrip() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Props {
            salad: String,
        }

        let props = Props {
            salad: "bar".to_string(),
        };

        let dna_hash_0 =
            DnaHashB64::from_b64_str("uhC0kAAD_AJfVAQBxgQHGAPQoAAHTATIAlQFk_7n_AQAB_-PDre2C")
                .unwrap();
        let dna_hash_1 =
            DnaHashB64::from_b64_str("uhC0kyiEBnw7_EsuRAAABcgH_w-zfAQ7_9gBs_wEAPJwBjf_cn8ta")
                .unwrap();
        let version = DnaVersionSpec::from(vec![dna_hash_0.clone(), dna_hash_1.clone()]);

        let cells = vec![CellManifest {
            nick: "nick".into(),
            dna: DnaManifest {
                location: Some(DnaLocation::Local {
                    path: PathBuf::from("/tmp/test.dna.gz"),
                }),
                properties: Some(YamlProperties::new(serde_yaml::to_value(props).unwrap())),
                uuid: Some("uuid".into()),
                version: Some(version),
                clone_limit: 50,
            },
            provisioning: Some(CellProvisioning::Create { deferred: false }),
        }];
        let manifest = AppManifest::V1(AppManifestV1 {
            name: "Test app".to_string(),
            description: "Serialization roundtrip test".to_string(),
            cells,
        });
        let manifest_yaml = serde_yaml::to_string(&manifest).unwrap();
        let manifest_roundtrip = serde_yaml::from_str(&manifest_yaml).unwrap();

        assert_eq!(manifest, manifest_roundtrip);

        let expected_yaml = format!(
            r#"---

manifest_version: "1"
name: "Test app"
description: "Serialization roundtrip test"
cells:
  - nick: "nick"
    provisioning:
      strategy: "create"
      deferred: false
    dna:
      path: /tmp/test.dna.gz
      version:
        - {}
        - {}
      clone_limit: 50
      uuid: uuid
      properties:
        salad: "bar"

        "#,
            dna_hash_0, dna_hash_1
        );
        let actual = serde_yaml::to_value(&manifest).unwrap();
        let expected: serde_yaml::Value = serde_yaml::from_str(&expected_yaml).unwrap();

        // Check a handful of fields. Order matters in YAML, so to check the
        // entire structure would be too fragile for testing.
        let fields = &[
            "cells[0].nick",
            "cells[0].provisioning.deferred",
            "cells[0].dna.version[1]",
            "cells[0].dna.properties",
        ];
        assert_eq!(actual.get(fields[0]), expected.get(fields[0]));
        assert_eq!(actual.get(fields[1]), expected.get(fields[1]));
        assert_eq!(actual.get(fields[2]), expected.get(fields[2]));
        assert_eq!(actual.get(fields[3]), expected.get(fields[3]));
    }
}
