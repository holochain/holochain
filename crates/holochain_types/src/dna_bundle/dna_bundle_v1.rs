use std::{collections::HashSet, path::PathBuf};

use holo_hash::DnaHash;

use crate::prelude::{CellNick, JsonProperties};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BundleManifest {
    /// Name of the bundle, just for context
    name: String,

    /// Version of bundle format
    version: u8,

    /// The Cells that make up this bundle
    cells: Vec<BundleCell>,
}

/// Description of a new or existing Cell referenced by this Bundle
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BundleCell {
    /// The CellNick which will be given to the installed Cell for this Dna.
    nick: CellNick,

    /// Where to find this Dna.
    location: Option<BundleDnaLocation>,

    /// Optional default properties. May be overridden during installation.
    properties: Option<JsonProperties>,

    /// The hash of the Dna.
    ///
    /// In "dev" mode (to be defined), the hash can be omitted when installing
    /// a bundle, since it may be frequently changing. Otherwise, it is required
    /// for "real" bundles.
    version: Option<DnaVersionSpec>,

    /// Determines whether or not a Cell will be created during installation.
    provisioning: Option<BundleCellProvisioning>,

    /// If true, allow the app to trigger cloning this DNA to create a new Cell
    /// on a distinct DHT network
    allow_cloning: Option<bool>,
}

/// Where to find this Dna.
/// If Local, the path may refer to a Dna which is bundled with the manifest,
/// or it may be to some other absolute or relative file path.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BundleDnaLocation {
    /// Get Dna from local filesystem
    Local(PathBuf),

    /// Get Dna from URL
    Url(String),
}

/// Defines a criterion for a DNA version to match against.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DnaVersionSpec(HashSet<DnaHash>);

impl DnaVersionSpec {
    /// Check if a DNA satisfies this version spec
    pub fn _matches(&self, hash: &DnaHash) -> bool {
        self.0.contains(hash)
    }
}

/// Rules to determine if and how a Cell will be created for this Dna
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BundleCellProvisioning {
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

impl Default for BundleCellProvisioning {
    fn default() -> Self {
        Self::Create
    }
}
