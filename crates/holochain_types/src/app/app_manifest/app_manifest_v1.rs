//! App Manifest format, version 1.
//!
//! NB: After stabilization, *do not modify this file*! Create a new version of
//! the spec and leave this one alone to maintain backwards compatibility.

use super::{
    app_manifest_validated::{AppManifestValidated, CellManifestValidated},
    error::{AppManifestError, AppManifestResult},
};
use crate::prelude::{CellNick, YamlProperties};
use holo_hash::{DnaHash, DnaHashB64};
use std::collections::HashMap;

pub type Uuid = String;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AppManifestV1 {
    /// Name of the App. This may be used as the installed_app_id.
    pub(super) name: String,

    /// Description of the app, just for context.
    pub(super) description: String,

    /// The Cell manifests that make up this app.
    pub(super) cells: Vec<CellManifest>,
}

/// Description of a new or existing Cell referenced by this Bundle
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CellManifest {
    /// The CellNick which will be given to the installed Cell for this Dna.
    pub(super) nick: CellNick,

    /// Determines if, how, and when a Cell will be provisioned.
    pub(super) provisioning: Option<CellProvisioning>,

    /// Declares where to find the DNA, and options to modify it before
    /// inclusion in a Cell
    pub(super) dna: AppDnaManifest,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AppDnaManifest {
    /// Where to find this Dna. To specify a DNA included in a hApp Bundle,
    /// use a local relative path that corresponds with the bundle structure.
    ///
    /// Note that since this is flattened,
    /// there is no actual "location" key in the manifest.
    #[serde(flatten)]
    pub(super) location: Option<mr_bundle::Location>,

    /// Optional default properties. May be overridden during installation.
    pub(super) properties: Option<YamlProperties>,

    /// Optional fixed UUID. May be overridden during installation.
    pub(super) uuid: Option<Uuid>,

    /// The versioning constraints for the DNA. Ensures that only a DNA that
    /// matches the version spec will be used.
    pub(super) version: Option<DnaVersionFlexible>,

    /// Allow up to this many "clones" to be created at runtime.
    /// Each runtime clone is created by the `CreateClone` strategy,
    /// regardless of the provisioning strategy set in the manifest.
    /// Default: 0
    #[serde(default)]
    pub(super) clone_limit: u32,
}

/// Allow the DNA version to be specified as a single hash, rather than a
/// singleton list. Just a convenience.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
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

pub type DnaLocation = mr_bundle::Location;

/// Defines a criterion for a DNA version to match against.
///
/// Currently we're using the most simple possible version spec: A list of
/// valid DnaHashes. The order of the list is from latest version to earliest.
/// In subsequent manifest versions, this will become more expressive.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
pub struct DnaVersionSpec(Vec<DnaHashB64>);

// NB: the following is likely to remain in the API for DnaVersionSpec
impl DnaVersionSpec {
    /// Check if a DNA satisfies this version spec
    pub fn _matches(&self, hash: DnaHash) -> bool {
        self.0.contains(&hash.into())
    }
}

// NB: the following is likely to be removed from the API for DnaVersionSpec
// after our versioning becomes more sophisticated

impl DnaVersionSpec {
    pub fn dna_hashes(&self) -> Vec<&DnaHashB64> {
        self.0.iter().collect()
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
    /// Convert this human-focused manifest into a validated, concise representation
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
                    let AppDnaManifest {
                        location,
                        properties,
                        version,
                        uuid,
                        clone_limit,
                    } = dna;
                    // Go from "flexible" enum into proper DnaVersionSpec.
                    let version = version.map(Into::into);
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
pub mod tests {
    use futures::future::join_all;

    use super::*;
    use crate::prelude::*;
    use crate::{app::app_manifest::AppManifest, prelude::DnaDef};
    use ::fixt::prelude::*;
    use std::path::PathBuf;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Props {
        salad: String,
    }

    pub async fn app_manifest_fixture<I: IntoIterator<Item = DnaDef>>(
        location: Option<mr_bundle::Location>,
        dnas: I,
    ) -> (AppManifest, Vec<DnaHashB64>) {
        let props = Props {
            salad: "bar".to_string(),
        };

        let hashes = join_all(
            dnas.into_iter()
                .map(|dna| async move { DnaHash::with_data(&dna).await.into() }),
        )
        .await;

        let version = DnaVersionSpec::from(hashes.clone()).into();

        let cells = vec![CellManifest {
            nick: "nick".into(),
            dna: AppDnaManifest {
                location,
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
        (manifest, hashes)
    }

    #[tokio::test]
    async fn manifest_v1_roundtrip() {
        let location = Some(mr_bundle::Location::Path(PathBuf::from("/tmp/test.dna.gz")));
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
            dna_hashes[0], dna_hashes[1]
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
