//! App Manifest format, version 1.
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

use super::{
    app_manifest_validated::{AppManifestValidated, AppSlotManifestValidated},
    error::{AppManifestError, AppManifestResult},
};
use crate::prelude::{SlotId, YamlProperties};
use holo_hash::{DnaHash, DnaHashB64};
use std::collections::HashMap;

/// Placeholder for a real UUID type
pub type Uuid = String;

/// Version 1 of the App manifest schema
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_builder::Builder,
)]
#[serde(rename_all = "snake_case")]
pub struct AppManifestV1 {
    /// Name of the App. This may be used as the installed_app_id.
    pub name: String,

    /// Description of the app, just for context.
    pub description: Option<String>,

    /// The Cell manifests that make up this app.
    pub slots: Vec<AppSlotManifest>,
}

/// Description of an app "slot" defined by this app.
/// Slots get filled according to the provisioning rules, as well as by
/// potential runtime clones.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AppSlotManifest {
    /// The SlotId which will be given to the installed Cell for this Dna.
    pub id: SlotId,

    /// Determines if, how, and when a Cell will be provisioned.
    pub provisioning: Option<CellProvisioning>,

    /// Declares where to find the DNA, and options to modify it before
    /// inclusion in a Cell
    pub dna: AppSlotDnaManifest,
}

impl AppSlotManifest {
    /// Create a sample AppSlotManifest as a template to be followed
    pub fn sample(id: SlotId) -> Self {
        Self {
            id,
            provisioning: Some(CellProvisioning::default()),
            dna: AppSlotDnaManifest::sample(),
        }
    }
}

/// The DNA portion of an app slot
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AppSlotDnaManifest {
    /// Where to find this Dna. To specify a DNA included in a hApp Bundle,
    /// use a local relative path that corresponds with the bundle structure.
    ///
    /// Note that since this is flattened,
    /// there is no actual "location" key in the manifest.
    #[serde(flatten)]
    pub location: Option<mr_bundle::Location>,

    /// Optional default properties. May be overridden during installation.
    pub properties: Option<YamlProperties>,

    /// Optional fixed UUID. May be overridden during installation.
    pub uuid: Option<Uuid>,

    /// The versioning constraints for the DNA. Ensures that only a DNA that
    /// matches the version spec will be used.
    pub version: Option<DnaVersionFlexible>,

    /// Allow up to this many "clones" to be created at runtime.
    /// Each runtime clone is created by the `CreateClone` strategy,
    /// regardless of the provisioning strategy set in the manifest.
    /// Default: 0
    #[serde(default)]
    pub(super) clone_limit: u32,
}

impl AppSlotDnaManifest {
    /// Create a sample AppSlotDnaManifest as a template to be followed
    pub fn sample() -> Self {
        Self {
            location: Some(mr_bundle::Location::Bundled(
                "./path/to/my/dnabundle.dna".into(),
            )),
            properties: None,
            uuid: None,
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
#[allow(missing_docs)]
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
            slots,
            description: _,
        } = self;
        let slots = slots
            .into_iter()
            .map(
                |AppSlotManifest {
                     id,
                     provisioning,
                     dna,
                 }| {
                    let AppSlotDnaManifest {
                        location,
                        properties,
                        version,
                        uuid,
                        clone_limit,
                    } = dna;
                    // Go from "flexible" enum into proper DnaVersionSpec.
                    let version = version.map(Into::into);
                    let validated = match provisioning.unwrap_or_default() {
                        CellProvisioning::Create { deferred } => AppSlotManifestValidated::Create {
                            deferred,
                            clone_limit,
                            location: Self::require(location, "slots.dna.(path|url)")?,
                            properties,
                            uuid,
                            version,
                        },
                        CellProvisioning::CreateClone { deferred } => {
                            AppSlotManifestValidated::CreateClone {
                                deferred,
                                clone_limit,
                                location: Self::require(location, "slots.dna.(path|url)")?,
                                properties,
                                version,
                            }
                        }
                        CellProvisioning::UseExisting { deferred } => {
                            AppSlotManifestValidated::UseExisting {
                                deferred,
                                clone_limit,
                                version: Self::require(version, "slots.dna.version")?,
                            }
                        }
                        CellProvisioning::CreateIfNotExists { deferred } => {
                            AppSlotManifestValidated::CreateIfNotExists {
                                deferred,
                                clone_limit,
                                location: Self::require(location, "slots.dna.(path|url)")?,
                                version: Self::require(version, "slots.dna.version")?,
                                properties,
                                uuid,
                            }
                        }
                        CellProvisioning::Disabled => AppSlotManifestValidated::Disabled {
                            clone_limit,
                            version: Self::require(version, "slots.dna.version")?,
                        },
                    };
                    Ok((id, validated))
                },
            )
            .collect::<Result<HashMap<_, _>, _>>()?;
        AppManifestValidated::new(name, slots)
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
                .map(|dna| async move { DnaHash::with_data_sync(&dna).into() }),
        )
        .await;

        let version = DnaVersionSpec::from(hashes.clone()).into();

        let slots = vec![AppSlotManifest {
            id: "nick".into(),
            dna: AppSlotDnaManifest {
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
            description: Some("Serialization roundtrip test".to_string()),
            slots,
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
slots:
  - id: "nick"
    provisioning:
      strategy: "create"
      deferred: false
    dna:
      path: /tmp/test.dna
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
            "slots[0].id",
            "slots[0].provisioning.deferred",
            "slots[0].dna.version[1]",
            "slots[0].dna.properties",
        ];
        assert_eq!(actual.get(fields[0]), expected.get(fields[0]));
        assert_eq!(actual.get(fields[1]), expected.get(fields[1]));
        assert_eq!(actual.get(fields[2]), expected.get(fields[2]));
        assert_eq!(actual.get(fields[3]), expected.get(fields[3]));
    }
}
