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
use crate::{dna::DnaBundle, properties::YamlProperties};
pub use app_bundle::*;
pub use app_manifest::app_manifest_validated::*;
pub use app_manifest::*;
use derive_more::Into;
pub use dna_gamut::*;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_serialized_bytes::prelude::*;
use holochain_util::ffs;
use holochain_zome_types::prelude::*;
use itertools::Itertools;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use self::error::{AppError, AppResult};

/// The unique identifier for an installed app in this conductor
pub type InstalledAppId = String;

/// A friendly (nick)name used by UIs to refer to the Cells which make up the app
#[deprecated = "Remove when InstallApp goes away; use SlotId instead"]
pub type CellNick = String;

/// Identifier for an AppSlot
pub type SlotId = String;

/// The source of the DNA to be installed, either as binary data, or from a path
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaSource {
    /// register the dna loaded from a bundle file on disk
    Path(PathBuf),
    /// register the dna as provided in the DnaBundle data structure
    Bundle(DnaBundle),
    /// register the dna from an existing registered DNA (assumes properties will be set)
    Hash(DnaHash),
}

/// The instructions on how to get the DNA to be registered
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RegisterDnaPayload {
    /// UID to override when installing this Dna
    pub uid: Option<String>,
    /// Properties to override when installing this Dna
    pub properties: Option<YamlProperties>,
    /// Where to find the DNA
    #[serde(flatten)]
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
    /// The SlotId under which to create this clone
    /// (needed to track cloning permissions and `clone_count`)
    pub slot_id: SlotId,
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
    #[serde(flatten)]
    pub source: AppBundleSource,

    /// The agent to use when creating Cells for this App.
    pub agent_key: AgentPubKey,

    /// The unique identifier for an installed app in this conductor.
    /// If not specified, it will be derived from the app name in the bundle manifest.
    pub installed_app_id: Option<InstalledAppId>,

    /// Include proof-of-membrane-membership data for cells that require it,
    /// keyed by the CellNick specified in the app bundle manifest.
    pub membrane_proofs: HashMap<CellNick, MembraneProof>,

    /// Optional: overwrites all UIDs for all DNAs of Cells created by this app.
    /// The app can still use existing Cells, i.e. this does not require that
    /// all Cells have DNAs with the same overridden DNA.
    pub uid: Option<Uid>,
}

/// The possible locations of an AppBundle
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppBundleSource {
    /// The actual serialized bytes of a bundle
    Bundle(AppBundle),
    /// A local file path
    Path(PathBuf),
    // /// A URL
    // Url(String),
}

impl AppBundleSource {
    /// Get the bundle from the source. Consumes the source.
    pub async fn resolve(self) -> Result<AppBundle, AppBundleError> {
        Ok(match self {
            Self::Bundle(bundle) => bundle,
            Self::Path(path) => AppBundle::decode(&ffs::read(&path).await?)?,
            // Self::Url(url) => todo!("reqwest::get"),
        })
    }
}

/// Information needed to specify a Dna as part of an App
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppDnaPayload {
    /// The hash of the DNA
    pub hash: DnaHash,
    /// The CellNick which will be assigned to this Dna when installed
    pub nick: CellNick,
    /// App-specific proof-of-membrane-membership, if required by this app
    pub membrane_proof: Option<MembraneProof>,
}

impl InstallAppDnaPayload {
    /// Create a payload from hash. Good for tests.
    pub fn hash_only(hash: DnaHash, nick: CellNick) -> Self {
        Self {
            hash,
            nick,
            membrane_proof: None,
        }
    }
}

/// Data about an installed Cell. It's deprecated because it is not used in
/// the new installation scheme using AppBundles.
#[deprecated = "can be removed after the old way of installing apps (`InstallApp`) is phased out"]
#[derive(Clone, Debug, Into, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InstalledCell {
    cell_id: CellId,
    // TODO: rename to slot_id
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

/// An app which has been installed.
/// An installed app is merely its collection of "slots", associated with an ID.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
pub enum InstalledApp {
    /// An active app
    Active(ActiveApp),
    /// An inactive app
    Inactive(InactiveApp),
}

impl InstalledApp {
    /// Constructor for freshly installed app
    pub fn new_inactive(app: InstalledAppCommon) -> Self {
        Self::Inactive(InactiveApp {
            app,
            reason: DeactivationReason::NeverActivated,
        })
    }

    /// Constructor for freshly installed app
    pub fn new_active(app: InstalledAppCommon) -> Self {
        Self::Active(ActiveApp(app))
    }

    /// Return the common app info, as well as a status which encodes the remaining
    /// information
    pub fn into_app_and_status(self) -> (InstalledAppCommon, InstalledAppStatus) {
        match self {
            Self::Active(app) => (app.into_common(), InstalledAppStatus::Active),
            Self::Inactive(InactiveApp { app, reason, .. }) => {
                (app, InstalledAppStatus::Inactive { reason })
            }
        }
    }

    /// Return the status
    pub fn status(&self) -> InstalledAppStatus {
        match self {
            Self::Active(_) => InstalledAppStatus::Active,
            Self::Inactive(InactiveApp { reason, .. }) => InstalledAppStatus::Inactive {
                reason: reason.clone(),
            },
        }
    }
}

impl AsRef<InstalledAppCommon> for InstalledApp {
    fn as_ref(&self) -> &InstalledAppCommon {
        match self {
            Self::Active(app) => &app.0,
            Self::Inactive(app) => &app.app,
        }
    }
}

impl std::ops::Deref for InstalledApp {
    type Target = InstalledAppCommon;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

/// An active app
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    derive_more::From,
    shrinkwraprs::Shrinkwrap,
)]
pub struct ActiveApp(InstalledAppCommon);

impl ActiveApp {
    /// Convert to a InactiveApp with the given reason
    pub fn into_inactive(self, reason: DeactivationReason) -> InactiveApp {
        InactiveApp::new(self.0, reason)
    }

    /// Add a cloned cell
    pub fn add_clone(&mut self, slot_id: &SlotId, cell_id: CellId) -> AppResult<()> {
        let slot = self.0.slot_mut(slot_id)?;
        assert_eq!(
            cell_id.agent_pubkey(),
            slot.agent_key(),
            "A clone cell must use the same agent key as the slot it is added to"
        );
        if slot.clones.len() as u32 >= slot.clone_limit {
            return Err(AppError::CloneLimitExceeded(slot.clone_limit, slot.clone()));
        }
        let _ = slot.clones.insert(cell_id);
        Ok(())
    }

    /// Remove a cloned cell
    pub fn remove_clone(&mut self, slot_id: &SlotId, cell_id: &CellId) -> AppResult<bool> {
        let slot = self.0.slot_mut(slot_id)?;
        Ok(slot.clones.remove(cell_id))
    }

    /// Move inner type out
    pub fn into_common(self) -> InstalledAppCommon {
        self.0
    }
}

impl automap::AutoMapped for ActiveApp {
    type Key = InstalledAppId;

    fn key(&self) -> &Self::Key {
        &self.0.installed_app_id
    }
}

/// An app which has either never been activated, or has been deactivated.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, shrinkwraprs::Shrinkwrap,
)]
pub struct InactiveApp {
    #[shrinkwrap(main_field)]
    app: InstalledAppCommon,
    reason: DeactivationReason,
}

impl InactiveApp {
    /// Constructor
    pub fn new(app: InstalledAppCommon, reason: DeactivationReason) -> Self {
        Self { app, reason }
    }

    /// Constructor
    pub fn new_fresh(app: InstalledAppCommon) -> Self {
        Self {
            app,
            reason: DeactivationReason::NeverActivated,
        }
    }

    /// Convert to a ActiveApp
    pub fn into_active(self) -> ActiveApp {
        ActiveApp(self.app)
    }

    /// Move inner type out
    pub fn into_common(self) -> InstalledAppCommon {
        self.app
    }
}

impl automap::AutoMapped for InactiveApp {
    type Key = InstalledAppId;

    fn key(&self) -> &Self::Key {
        &self.app.installed_app_id
    }
}

impl From<InactiveApp> for InstalledAppCommon {
    fn from(d: InactiveApp) -> Self {
        d.app
    }
}

/// The possible reasons for an app being deactivated
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeactivationReason {
    /// The app has never been fully activated, and is just awaiting genesis
    NeverActivated,
    /// The app was deactivated by the user normally
    Normal,
    /// The app was automatically deactivated due to an unrecoverable error by
    /// one of its Cells
    Quarantined {
        /// The error which necessitated the quarantine
        error: String,
    },
}

/// The common data between apps of any status
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledAppCommon {
    /// The unique identifier for an installed app in this conductor
    installed_app_id: InstalledAppId,
    /// The agent key used to install this app. Currently this is meaningless,
    /// but I'm leaving it here as a placeholder in case we ever want it to
    /// have formal significance.
    _agent_key: AgentPubKey,
    /// The "slots" as specified in the AppManifest
    slots: HashMap<CellNick, AppSlot>,
}

impl InstalledAppCommon {
    /// Constructor
    pub fn new<S: ToString, I: IntoIterator<Item = (SlotId, AppSlot)>>(
        installed_app_id: S,
        _agent_key: AgentPubKey,
        slots: I,
    ) -> Self {
        InstalledAppCommon {
            installed_app_id: installed_app_id.to_string(),
            _agent_key,
            slots: slots.into_iter().collect(),
        }
    }

    /// Accessor
    pub fn installed_app_id(&self) -> &InstalledAppId {
        &self.installed_app_id
    }

    /// Accessor
    pub fn provisioned_cells(&self) -> impl Iterator<Item = (&SlotId, &CellId)> {
        self.slots
            .iter()
            .filter_map(|(nick, slot)| slot.provisioned_cell().map(|c| (nick, c)))
    }

    /// Accessor
    pub fn into_provisioned_cells(self) -> impl Iterator<Item = (SlotId, CellId)> {
        self.slots
            .into_iter()
            .filter_map(|(nick, slot)| slot.into_provisioned_cell().map(|c| (nick, c)))
    }

    /// Accessor
    pub fn cloned_cells(&self) -> impl Iterator<Item = &CellId> {
        self.slots.iter().map(|(_, slot)| &slot.clones).flatten()
    }

    /// Iterator of all cells, both provisioned and cloned
    pub fn all_cells(&self) -> impl Iterator<Item = &CellId> {
        self.provisioned_cells()
            .map(|(_, c)| c)
            .chain(self.cloned_cells())
    }

    /// Accessor for particular slot
    pub fn slot(&self, slot_id: &SlotId) -> AppResult<&AppSlot> {
        self.slots
            .get(slot_id)
            .ok_or_else(|| AppError::SlotIdMissing(slot_id.clone()))
    }

    fn slot_mut(&mut self, slot_id: &SlotId) -> AppResult<&mut AppSlot> {
        self.slots
            .get_mut(slot_id)
            .ok_or_else(|| AppError::SlotIdMissing(slot_id.clone()))
    }

    /// Accessor
    pub fn slots(&self) -> &HashMap<SlotId, AppSlot> {
        &self.slots
    }

    /// Accessor
    pub fn _agent_key(&self) -> &AgentPubKey {
        &self._agent_key
    }

    /// Constructor for apps not using a manifest.
    /// Disables cloning, and implies immediate provisioning.
    pub fn new_legacy<S: ToString, I: IntoIterator<Item = InstalledCell>>(
        installed_app_id: S,
        installed_cells: I,
    ) -> AppResult<Self> {
        let installed_app_id = installed_app_id.to_string();
        let installed_cells: Vec<_> = installed_cells.into_iter().collect();

        // Get the agent key of the first cell
        // NB: currently this has no significance.
        let _agent_key = installed_cells
            .get(0)
            .expect("Can't create app with 0 cells")
            .cell_id
            .agent_pubkey()
            .to_owned();

        // ensure all cells use the same agent key
        if installed_cells
            .iter()
            .any(|c| *c.cell_id.agent_pubkey() != _agent_key)
        {
            tracing::warn!(
                        "It's kind of an informal convention that all cells in a legacy installation should use the same agent key. But, no big deal... Cell data: {:#?}",
                        installed_cells
                    );
        }

        // ensure all cells use the same agent key
        let duplicates: Vec<CellNick> = installed_cells
            .iter()
            .map(|c| c.cell_nick.to_owned())
            .counts()
            .into_iter()
            .filter_map(|(nick, count)| if count > 1 { Some(nick) } else { None })
            .collect();
        if !duplicates.is_empty() {
            return Err(AppError::DuplicateSlotIds(installed_app_id, duplicates));
        }

        let slots = installed_cells
            .into_iter()
            .map(|InstalledCell { cell_nick, cell_id }| {
                let slot = AppSlot {
                    base_cell_id: cell_id,
                    is_provisioned: true,
                    clones: HashSet::new(),
                    clone_limit: 0,
                };
                (cell_nick, slot)
            })
            .collect();
        Ok(Self {
            installed_app_id,
            _agent_key,
            slots,
        })
    }
}

/// The information in an InstalledApp which is not captured by InstalledAppCommon
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum InstalledAppStatus {
    /// Indicates the app is active
    Active,
    /// Indicates the app is inactive, and why
    Inactive {
        /// The reason for deactivation
        reason: DeactivationReason,
    },
}

/// A map from InstalledAppId -> ActiveApp
pub type InstalledAppMap = automap::AutoHashMap<ActiveApp>;
/// A map from InstalledAppId -> InactiveApp
pub type DeactivatedAppMap = automap::AutoHashMap<InactiveApp>;

/// Cell "slots" correspond to cell entries in the AppManifest.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppSlot {
    /// The Id of the Cell which will be provisioned for this slot.
    /// This also identifies the basis for cloned DNAs, and this is how the
    /// Agent is determined for clones (always the same as the provisioned cell).
    base_cell_id: CellId,
    /// Records whether the base cell has actually been provisioned or not.
    /// If true, then `base_cell_id` refers to an actual existing Cell.
    /// If false, then `base_cell_id` is just recording what that cell will be
    /// called in the future.
    is_provisioned: bool,
    /// The number of cloned cells allowed
    clone_limit: u32,
    /// Cells which were cloned at runtime. The length cannot grow beyond
    /// `clone_limit`
    clones: HashSet<CellId>,
}

impl AppSlot {
    /// Constructor. List of clones always starts empty.
    pub fn new(base_cell_id: CellId, is_provisioned: bool, clone_limit: u32) -> Self {
        Self {
            base_cell_id,
            is_provisioned,
            clone_limit,
            clones: HashSet::new(),
        }
    }

    /// Accessor
    pub fn cell_id(&self) -> &CellId {
        &self.base_cell_id
    }

    /// Accessor
    pub fn dna_hash(&self) -> &DnaHash {
        &self.base_cell_id.dna_hash()
    }

    /// Accessor
    pub fn agent_key(&self) -> &AgentPubKey {
        &self.base_cell_id.agent_pubkey()
    }

    /// Accessor
    pub fn provisioned_cell(&self) -> Option<&CellId> {
        if self.is_provisioned {
            Some(&self.base_cell_id)
        } else {
            None
        }
    }

    /// Transformer
    pub fn into_provisioned_cell(self) -> Option<CellId> {
        if self.is_provisioned {
            Some(self.base_cell_id)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ActiveApp, AppSlot};
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use std::collections::HashSet;

    #[test]
    fn clone_management() {
        let base_cell_id = fixt!(CellId);
        let agent = base_cell_id.agent_pubkey().clone();
        let new_clone = || CellId::new(fixt!(DnaHash), agent.clone());
        let slot1 = AppSlot::new(base_cell_id, false, 3);
        let agent = fixt!(AgentPubKey);
        let slot_id: SlotId = "slot_id".into();
        let mut app: ActiveApp =
            InstalledAppCommon::new("app", agent.clone(), vec![(slot_id.clone(), slot1)]).into();

        // Can add clones up to the limit
        let clones: Vec<_> = vec![new_clone(), new_clone(), new_clone()];
        app.add_clone(&slot_id, clones[0].clone()).unwrap();
        app.add_clone(&slot_id, clones[1].clone()).unwrap();
        app.add_clone(&slot_id, clones[2].clone()).unwrap();

        // Adding a clone beyond the clone_limit is an error
        matches::assert_matches!(
            app.add_clone(&slot_id, new_clone()),
            Err(AppError::CloneLimitExceeded(3, _))
        );

        assert_eq!(
            app.cloned_cells().collect::<HashSet<_>>(),
            maplit::hashset! { &clones[0], &clones[1], &clones[2] }
        );

        assert_eq!(app.remove_clone(&slot_id, &clones[1]).unwrap(), true);
        assert_eq!(app.remove_clone(&slot_id, &clones[1]).unwrap(), false);

        assert_eq!(
            app.cloned_cells().collect::<HashSet<_>>(),
            maplit::hashset! { &clones[0], &clones[2] }
        );

        // Adding the same clone twice should probably be a panic, but if this
        // line is still here, I never got around to making it panic...
        app.add_clone(&slot_id, clones[0].clone()).unwrap();

        assert_eq!(app.cloned_cells().count(), 2);

        assert_eq!(
            app.cloned_cells().collect::<HashSet<_>>(),
            app.all_cells().collect::<HashSet<_>>()
        );
    }

    #[test]
    fn status_serialization() {
        let status = InstalledAppStatus::Inactive {
            reason: DeactivationReason::Quarantined {
                error: "because".into(),
            },
        };

        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(
            json,
            "{\"inactive\":{\"reason\":{\"quarantined\":{\"error\":\"because\"}}}}"
        );
    }
}
