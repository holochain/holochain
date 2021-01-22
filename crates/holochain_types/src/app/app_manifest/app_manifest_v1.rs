use std::path::PathBuf;

use holo_hash::DnaHash;

use crate::prelude::{CellNick, YamlProperties};

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

    /// Where to find this Dna.
    #[serde(flatten)]
    location: Option<DnaLocation>,

    /// Optional default properties. May be overridden during installation.
    properties: Option<YamlProperties>,

    /// The hash of the Dna.
    ///
    /// In "dev" mode (to be defined), the hash can be omitted when installing
    /// a bundle, since it may be frequently changing. Otherwise, it is required
    /// for "real" bundles.
    version: Option<DnaVersionSpec>,

    /// Determines whether or not a Cell will be created during installation.
    provisioning: Option<CellProvisioning>,

    /// If true, allow the app to trigger cloning this DNA to create a new Cell
    /// on a distinct DHT network
    allow_cloning: Option<bool>,
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
pub struct DnaVersionSpec(Vec<DnaHash>);

impl DnaVersionSpec {
    /// Check if a DNA satisfies thYamlPropertiesis version spec
    pub fn _matches(&self, hash: &DnaHash) -> bool {
        self.0.contains(hash)
    }
}

/// Rules to determine if and how a Cell will be created for this Dna
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellProvisioning {
    /// Always create a new Cell when installing this App
    Create,
    /// Always create a new Cell when installing the App,
    /// and use a unique UUID to ensure a distinct DHT network
    CreateUnique,
    /// Require that a Cell is already installed which matches the DNA version
    /// spec, and which has an Agent that's associated with this App's agent
    /// via DPKI. If no such Cell exists, *app installation fails*.
    UseExisting,
    /// Try `UseExisting`, and if that fails, fallback to `Create`
    CreateIfNotExists,
    /// Don't install a Cell at all during App installation.
    /// This indicates that a Dna is only meant to be "cloned" by the app.
    DoNothing,
}

impl Default for CellProvisioning {
    fn default() -> Self {
        Self::Create
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::app_manifest::AppManifest;
    use crate::prelude::YamlProperties;
    use ::fixt::prelude::*;
    use holochain_zome_types::DnaHashFixturator;
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
        let version = vec![fixt!(DnaHash), fixt!(DnaHash)];

        let cells = vec![CellManifest {
            nick: "nick".into(),
            location: Some(DnaLocation::Local {
                path: PathBuf::from("/tmp/test.dna.gz"),
            }),
            properties: Some(YamlProperties::new(serde_yaml::to_value(props).unwrap())),
            version: Some(version.into()),
            provisioning: Some(CellProvisioning::Create),
            allow_cloning: Some(false),
        }];
        let manifest = AppManifest::V1(AppManifestV1 {
            name: "Test app".to_string(),
            description: "Serialization roundtrip test".to_string(),
            cells,
        });
        let manifest_yaml = serde_yaml::to_string(&manifest).unwrap();
        let manifest_roundtrip = serde_yaml::from_str(&manifest_yaml).unwrap();
        assert_eq!(manifest, manifest_roundtrip);

        //         let manifest_yaml = r#"---
        // manifest_version: 1
        // name: "Test app"
        // description: "Serialization roundtrip test"
        // cells: []
        //   - nick: "cell-1"
        //     provisioning:
        //       strategy: "create"
        //       deferred: no
        //     dna:
        //       path: /tmp/test.dna.gz
        //       version:
        //         - "HcDxx"
        //         - "HcDxy"
        //       clone_limit: 50
        //       properties:
        //         salad: "bar"
        //         foo: "fighters"

        //         "#;
    }
}
