//! Everything to do with App (hApp) installation and uninstallation
//!
//! An App is a essentially a collection of Cells which are intended to be
//! available for a particular Holochain use-case, such as a microservice used
//! by some UI in a broader application.
//!
//! Each Cell maintains its own identity separate from any App.
//! Access to Cells can be shared between different Apps.

mod app_bundle;
mod app_manifest;
mod dna_gamut;
pub mod error;
pub use app_bundle::*;
pub use app_manifest::app_manifest_validated::*;
pub use app_manifest::*;
pub use dna_gamut::*;

use crate::dna::{DnaFile, YamlProperties};
use derive_more::Into;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_serialized_bytes::SerializedBytes;
use holochain_zome_types::cell::CellId;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use self::error::{AppError, AppResult};

/// The unique identifier for an installed app in this conductor
pub type InstalledAppId = String;

/// A friendly (nick)name used by UIs to refer to the Cells which make up the app
pub type CellNick = String;

/// The source of the DNA to be installed, either as binary data, or from a path
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaSource {
    /// register the dna loaded from a file on disk
    Path(PathBuf),
    /// register the dna as provided in the DnaFile data structure
    DnaFile(DnaFile),
    /// register the dna from an existing registered DNA (assumes properties will be set)
    Hash(DnaHash),
}

/// The instructions on how to get the DNA to be registered
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RegisterDnaPayload {
    /// UUID to override when installing this Dna
    pub uuid: Option<String>,
    /// Properties to override when installing this Dna
    pub properties: Option<YamlProperties>,
    /// Where to find the DNA
    pub source: DnaSource,
}

/// The instructions on how to get the DNA to be registered
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CreateCloneCellPayload {
    /// Properties to override when installing this Dna
    pub properties: Option<YamlProperties>,
    /// The DNA to clone
    pub dna_hash: DnaHash,
    /// The Agent key with which to create this Cell
    /// (TODO: should this be derived from the App?)
    pub agent_key: AgentPubKey,
    /// The App with which to associate the newly created Cell
    pub installed_app_id: InstalledAppId,
    /// The CellNick under which to create this clone
    /// (needed to track cloning permissions and `clone_count`)
    pub cell_nick: CellNick,
    /// Proof-of-membership, if required by this DNA
    pub membrane_proof: Option<MembraneProof>,
}

impl CreateCloneCellPayload {
    /// Get the CellId of the to-be-created clone cell
    pub fn cell_id(&self) -> CellId {
        CellId::new(self.dna_hash.clone(), self.agent_key.clone())
    }
}

/// A collection of [DnaHash]es paired with an [AgentPubKey] and an app id
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppPayload {
    /// The unique identifier for an installed app in this conductor
    pub installed_app_id: InstalledAppId,

    /// The agent to use when creating Cells for this App
    pub agent_key: AgentPubKey,

    /// The Dna paths in this app
    pub dnas: Vec<InstallAppDnaPayload>,
}

/// An [AppBundle] along with an [AgentPubKey] and optional [InstalledAppId]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppBundlePayload {
    /// The unique identifier for an installed app in this conductor.
    pub bundle: AppBundle,

    /// The agent to use when creating Cells for this App.
    pub agent_key: AgentPubKey,

    /// The unique identifier for an installed app in this conductor.
    /// If not specified, it will be derived from the app name in the bundle manifest.
    pub installed_app_id: Option<InstalledAppId>,

    /// Include proof-of-membrane-membership data for cells that require it,
    /// keyed by the CellNick specified in the app bundle manifest.
    pub membrane_proofs: HashMap<CellNick, MembraneProof>,
}

/// Information needed to specify a Dna as part of an App
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppDnaPayload {
    /// The path of the DnaFile
    pub path: Option<PathBuf>, // This is a deprecated field should register dna contents via register
    /// The path of the DnaFile
    pub hash: Option<DnaHash>, // When path is removed, this will become non-opt
    /// The CellNick which will be assigned to this Dna when installed
    pub nick: CellNick,
    /// Properties to override when installing this Dna
    pub properties: Option<YamlProperties>,
    /// App-specific proof-of-membrane-membership, if required by this app
    pub membrane_proof: Option<MembraneProof>,
}

impl InstallAppDnaPayload {
    /// Create a payload with no YamlProperties or MembraneProof. Good for tests.
    pub fn path_only(path: PathBuf, nick: CellNick) -> Self {
        Self {
            path: Some(path),
            hash: None,
            nick,
            properties: None,
            membrane_proof: None,
        }
    }
    /// Create a payload with no JsonProperties or MembraneProof. Good for tests.
    pub fn hash_only(hash: DnaHash, nick: CellNick) -> Self {
        Self {
            path: None,
            hash: Some(hash),
            nick,
            properties: None,
            membrane_proof: None,
        }
    }
}

/// App-specific payload for proving membership in the membrane of the app
pub type MembraneProof = SerializedBytes;

/// Data about an installed Cell. It's deprecated because it is not used in
/// the new installation scheme using AppBundles.
#[deprecated = "can be removed after the old way of installing apps (`InstallApp`) is phased out"]
#[derive(Clone, Debug, Into, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InstalledCell {
    cell_id: CellId,
    cell_nick: CellNick,
}

impl InstalledCell {
    /// Constructor
    pub fn new(cell_id: CellId, cell_nick: CellNick) -> Self {
        Self { cell_id, cell_nick }
    }

    /// Get the CellId
    pub fn into_id(self) -> CellId {
        self.cell_id
    }

    /// Get the CellNick
    pub fn into_nick(self) -> CellNick {
        self.cell_nick
    }

    /// Get the inner data as a tuple
    pub fn into_inner(self) -> (CellId, CellNick) {
        (self.cell_id, self.cell_nick)
    }

    /// Get the CellId
    pub fn as_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Get the CellNick
    pub fn as_nick(&self) -> &CellNick {
        &self.cell_nick
    }
}

/// An installed app is merely its collection of "slots", associated with an ID.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledApp {
    /// The unique identifier for an installed app in this conductor
    installed_app_id: InstalledAppId,
    /// The "slots" as specified in the AppManifest
    slots: HashMap<CellNick, AppSlot>,
}

impl automap::AutoMapped for InstalledApp {
    type Key = InstalledAppId;

    fn key(&self) -> &Self::Key {
        &self.installed_app_id
    }
}

/// A map from InstalledAppId -> InstalledApp
pub type InstalledAppMap = automap::AutoHashMap<InstalledApp>;

impl InstalledApp {
    /// Constructor
    pub fn new<S: ToString, I: IntoIterator<Item = (CellNick, AppSlot)>>(
        installed_app_id: S,
        slots: I,
    ) -> Self {
        Self {
            installed_app_id: installed_app_id.to_string(),
            slots: slots.into_iter().collect(),
        }
    }

    /// Constructor for apps not using a manifest.
    /// Disables cloning, and implies immediate provisioning.
    pub fn new_legacy<S: ToString, I: IntoIterator<Item = InstalledCell>>(
        installed_app_id: S,
        installed_cells: I,
    ) -> Self {
        let installed_app_id = installed_app_id.to_string();
        let slots = installed_cells
            .into_iter()
            .map(|InstalledCell { cell_nick, cell_id }| {
                let slot = AppSlot {
                    provisioned_cell: Some(cell_id),
                    clones: HashSet::new(),
                    clone_limit: 0,
                };
                (cell_nick, slot)
            })
            .collect();
        Self {
            installed_app_id,
            slots,
        }
    }

    /// Accessor
    pub fn installed_app_id(&self) -> &InstalledAppId {
        &self.installed_app_id
    }

    /// Accessor
    pub fn provisioned_cells(&self) -> impl Iterator<Item = (&CellNick, &CellId)> {
        self.slots
            .iter()
            .filter_map(|(nick, slot)| slot.provisioned_cell.as_ref().map(|c| (nick, c)))
    }

    /// Accessor
    pub fn into_provisioned_cells(self) -> impl Iterator<Item = (CellNick, CellId)> {
        self.slots
            .into_iter()
            .filter_map(|(nick, slot)| slot.provisioned_cell.map(|c| (nick, c)))
    }

    /// Accessor
    pub fn cloned_cells(&self) -> impl Iterator<Item = &CellId> {
        self.slots.iter().map(|(_, slot)| &slot.clones).flatten()
    }

    /// Iterator of all cells, both provisioned and cloned
    pub fn cells(&self) -> impl Iterator<Item = &CellId> {
        self.provisioned_cells()
            .map(|(_, c)| c)
            .chain(self.cloned_cells())
    }

    /// Add a cloned cell
    pub fn add_clone(
        &mut self,
        cell_nick: &CellNick,
        properties: YamlProperties,
    ) -> AppResult<CellId> {
        let slot = self
            .slots
            .get_mut(cell_nick)
            .ok_or_else(|| AppError::CellNickMissing(cell_nick.clone()))?;
        if slot.clones.len() as u32 >= slot.clone_limit {
            return Err(AppError::CloneLimitExceeded(slot.clone_limit, slot.clone()));
        }
        let cell_id = todo!("Create cell with new UUID and specified properties");
        // cell_id is guaranteed to be unique
        let _ = slot.clones.insert(cell_id);
        Ok(cell_id)
    }

    /// Remove a cloned cell
    pub fn remove_clone(&mut self, cell_nick: &CellNick, cell_id: &CellId) -> AppResult<bool> {
        let slot = self
            .slots
            .get_mut(cell_nick)
            .ok_or_else(|| AppError::CellNickMissing(cell_nick.clone()))?;
        Ok(slot.clones.remove(cell_id))
    }
}

/// Cell "slots" correspond to cell entries in the AppManifest.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppSlot {
    /// A cells which was provisioned at install-time.
    /// If provisioning was deferred, this will be None, and will become Some
    /// once the cell is created.
    provisioned_cell: Option<CellId>,
    /// The number of cloned cells allowed
    clone_limit: u32,
    /// Cells which were cloned at runtime. The length cannot grow beyond
    /// `clone_limit`
    clones: HashSet<CellId>,
}

impl AppSlot {
    /// Constructor. List of clones always starts empty.
    pub fn new(provisioned_cell: Option<CellId>, clone_limit: u32) -> Self {
        Self {
            provisioned_cell,
            clone_limit,
            clones: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppSlot, InstalledApp};
    use crate::prelude::*;
    use std::collections::HashSet;

    #[test]
    fn clone_management() {
        let slot1 = AppSlot::new(None, 3);
        let nick: CellNick = "nick".into();
        let mut app = InstalledApp::new("app", vec![(nick.clone(), slot1)]);

        // Can add clones up to the limit
        let clone_id_0 = app.add_clone(&nick, ().into()).unwrap();
        let clone_id_1 = app.add_clone(&nick, ().into()).unwrap();
        let clone_id_2 = app.add_clone(&nick, ().into()).unwrap();

        assert_eq!(
            maplit::hashset! { &clone_id_0, &clone_id_1, &clone_id_2}.len(),
            3,
            "All clone IDs are unique"
        );

        // Adding a clone beyond the clone_limit is an error
        matches::assert_matches!(
            app.add_clone(&nick, ().into()),
            Err(AppError::CloneLimitExceeded(3, _))
        );

        assert_eq!(
            app.cloned_cells().collect::<HashSet<_>>(),
            maplit::hashset! { &clone_id_0, &clone_id_1, &clone_id_2 },
            "All CellIds are present"
        );

        assert_eq!(app.remove_clone(&nick, &clone_id_1).unwrap(), true);
        assert_eq!(app.remove_clone(&nick, &clone_id_1).unwrap(), false);

        assert_eq!(
            app.cloned_cells().collect::<HashSet<_>>(),
            maplit::hashset! { &clone_id_0, &clone_id_2 }
        );

        assert_eq!(app.cloned_cells().count(), 2);

        assert_eq!(
            app.cloned_cells().collect::<HashSet<_>>(),
            app.cells().collect::<HashSet<_>>()
        );
    }
}
