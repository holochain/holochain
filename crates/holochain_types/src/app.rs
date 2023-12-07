//! Everything to do with App (hApp) installation and uninstallation
//!
//! An App is a essentially a collection of Cells which are intended to be
//! available for a particular Holochain use-case, such as a microservice used
//! by some UI in a broader application.
//!
//! Each Cell maintains its own identity separate from any App.
//! Access to Cells can be shared between different Apps.

use holochain_app::{AppBundle, AppBundleResult};
use holochain_dna_types::prelude::*;

pub use error::*;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_util::ffs;
use std::{collections::HashMap, path::PathBuf};

/// Alias
pub type InstalledAppId = holochain_app::AppId;

/// The source of the DNA to be installed, either as binary data, or from a path
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaSource {
    /// register the dna loaded from a bundle file on disk
    Path(PathBuf),
    /// register the dna as provided in the DnaBundle data structure
    Bundle(Box<DnaBundle>),
    /// register the dna from an existing registered DNA (assumes properties will be set)
    Hash(DnaHash),
}

/// The source of coordinators to be installed, either as binary data, or from a path
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinatorSource {
    /// Coordinators loaded from a bundle file on disk
    Path(PathBuf),
    /// Coordinators provided in the [`CoordinatorBundle`] data structure
    Bundle(Box<CoordinatorBundle>),
}

/// The instructions on how to get the DNA to be registered
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RegisterDnaPayload {
    /// Modifier overrides
    #[serde(default)]
    pub modifiers: DnaModifiersOpt<YamlProperties>,
    /// Where to find the DNA
    #[serde(flatten)]
    pub source: DnaSource,
}

/// The instructions on how to request NetworkInfo
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkInfoRequestPayload {
    /// The calling agent
    pub agent_pub_key: AgentPubKey,
    /// Get gossip info for these DNAs
    pub dnas: Vec<DnaHash>,
    /// Timestamp in ms since which received amount of bytes from peers will
    /// be returned. Defaults to UNIX_EPOCH.
    pub last_time_queried: Option<Timestamp>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
/// The instructions on how to update coordinators for a dna file.
pub struct UpdateCoordinatorsPayload {
    /// The hash of the dna to swap coordinators for.
    pub dna_hash: DnaHash,
    /// Where to find the coordinators.
    #[serde(flatten)]
    pub source: CoordinatorSource,
}

/// The arguments to create a clone of an existing cell.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CreateCloneCellPayload {
    /// The app id that the DNA to clone belongs to
    pub app_id: InstalledAppId,
    /// The DNA's role name to clone
    pub role_name: RoleName,
    /// Modifiers to set for the new cell.
    /// At least one of the modifiers must be set to obtain a distinct hash for
    /// the clone cell's DNA.
    pub modifiers: DnaModifiersOpt<YamlProperties>,
    /// Optionally set a proof of membership for the clone cell
    pub membrane_proof: Option<MembraneProof>,
    /// Optionally a name for the DNA clone
    pub name: Option<String>,
}

/// Arguments to specify the clone cell to be disabled.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DisableCloneCellPayload {
    /// The app id that the clone cell belongs to
    pub app_id: InstalledAppId,
    /// The clone id or cell id of the clone cell
    pub clone_cell_id: holochain_app::CloneCellId,
}

/// Argumtents to specify the clone cell to be enabled.
pub type EnableCloneCellPayload = DisableCloneCellPayload;

/// Arguments to delete a disabled clone cell of an app.
pub type DeleteCloneCellPayload = DisableCloneCellPayload;

/// An [AppBundle] along with an [AgentPubKey] and optional [InstalledAppId]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppPayload {
    /// The unique identifier for an installed app in this conductor.
    #[serde(flatten)]
    pub source: AppBundleSource,

    /// The agent to use when creating Cells for this App.
    pub agent_key: AgentPubKey,

    /// The unique identifier for an installed app in this conductor.
    /// If not specified, it will be derived from the app name in the bundle manifest.
    pub installed_app_id: Option<InstalledAppId>,

    /// Include proof-of-membrane-membership data for cells that require it,
    /// keyed by the RoleName specified in the app bundle manifest.
    pub membrane_proofs: HashMap<RoleName, MembraneProof>,

    /// Optional: overwrites all network seeds for all DNAs of Cells created by this app.
    /// The app can still use existing Cells, i.e. this does not require that
    /// all Cells have DNAs with the same overridden DNA.
    pub network_seed: Option<NetworkSeed>,

    /// Optional: If app installation fails due to genesis failure, normally the app will be
    /// immediately uninstalled. When this flag is set, the app is left installed with empty cells intact.
    /// This can be useful for using `graft_records_onto_source_chain`, or for diagnostics.
    #[cfg(feature = "chc")]
    #[serde(default)]
    pub ignore_genesis_failure: bool,
}

/// The possible locations of an AppBundle
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppBundleSource {
    /// The actual serialized bytes of a bundle
    Bundle(holochain_app::AppBundle),
    /// A local file path
    Path(PathBuf),
    // /// A URL
    // Url(String),
}

impl AppBundleSource {
    /// Get the bundle from the source. Consumes the source.
    pub async fn resolve(self) -> AppBundleResult<AppBundle> {
        Ok(match self {
            Self::Bundle(bundle) => bundle,
            Self::Path(path) => AppBundle::decode(&ffs::read(&path).await?)?,
            // Self::Url(url) => todo!("reqwest::get"),
        })
    }
}

/// Information needed to specify a DNA as part of an App
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppDnaPayload {
    /// The hash of the DNA
    pub hash: DnaHash,
    /// The RoleName which will be assigned to this DNA when installed
    pub role_name: RoleName,
    /// App-specific proof-of-membrane-membership, if required by this app
    pub membrane_proof: Option<MembraneProof>,
}

impl InstallAppDnaPayload {
    /// Create a payload from hash. Good for tests.
    pub fn hash_only(hash: DnaHash, role_name: RoleName) -> Self {
        Self {
            hash,
            role_name,
            membrane_proof: None,
        }
    }
}
