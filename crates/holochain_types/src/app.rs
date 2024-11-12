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
mod error;

use crate::{dna::DnaBundle, prelude::*};
pub use app_bundle::*;
pub use app_manifest::app_manifest_validated::*;
pub use app_manifest::*;
use derive_more::Into;
pub use error::*;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_serialized_bytes::prelude::*;
use holochain_util::ffs;
use holochain_zome_types::cell::CloneId;
use holochain_zome_types::prelude::*;
use indexmap::IndexMap;
use std::{collections::HashMap, path::PathBuf};

/// The unique identifier for an installed app in this conductor
pub type InstalledAppId = String;

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
    // TODO should this be restricted to the agent for the current app?
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

/// The parameters to create a clone of an existing cell.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CreateCloneCellPayload {
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

/// Parameters to specify the clone cell to be disabled.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DisableCloneCellPayload {
    /// The clone id or cell id of the clone cell
    pub clone_cell_id: CloneCellId,
}

/// Parameters to specify the clone cell to be enabled.
pub type EnableCloneCellPayload = DisableCloneCellPayload;

/// Parameters to delete a disabled clone cell of an app.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DeleteCloneCellPayload {
    /// The app id that the DNA to clone belongs to
    pub app_id: InstalledAppId,

    /// The clone id or cell id of the clone cell
    pub clone_cell_id: CloneCellId,
}

/// All the information necessary to install an app
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppPayload {
    /// Where to obtain the AppBundle, which contains the app manifest and DNA bundles
    /// to be installed. This is the main payload of app installation.
    ///
    /// Since this field uses `#[serde(flatten)]`, when using other serialized data formats
    /// like JSON or YAML, this field will actually show up as one of the variants of
    /// `AppBundleSource` (e.g. `bundle` or `path`), rather than as a `source` field.
    #[serde(flatten)]
    pub source: AppBundleSource,

    /// The agent to use when creating Cells for this App.
    /// If None, a new agent key will be generated in the right circumstances (read on).
    ///
    /// It's always OK to provide a pregenerated agent key here, but there is at least one
    /// major benefit to letting Holochain generate keys for you (other than
    /// the sheer convenience of not having to generate your own):
    ///
    /// If you are using a device seed in your conductor config, the agent key will be derived
    /// from that seed using a sensible scheme based on the total number of app installations
    /// in this conductor, which means you can fairly easily regenerate all of your auto-generated
    /// agent keys if you lose access to the device with your conductor data
    /// (as long as you retain exclusive access to the device seed of course).
    ///
    /// Holochain will only generate an agent key for you if [`ConductorConfig::device_seed_lair_tag`]
    /// is set and pointing to a seed present in lair. If this config is not set, installation
    /// will fail if no agent key is provided. This safety mechanism can however be overridden
    /// by setting the `allow_throwaway_random_agent_key` flag on this payload, which will cause
    /// Holochain to generate a totally random (non-recoverable) agent key.
    ///
    /// If you are not using a device seed, or if your app has special requirements for agent keys,
    /// you can always provide your own here, no matter what setting you're using.
    #[serde(default)]
    pub agent_key: Option<AgentPubKey>,

    /// The unique identifier for an installed app in this conductor.
    /// If not specified, it will be derived from the app name in the bundle manifest.
    #[serde(default)]
    pub installed_app_id: Option<InstalledAppId>,

    /// Optional: Overwrites all network seeds for all DNAs of Cells created by this app.
    /// This has a lower precedence than role-specific network seeds provided in the  `role_settings` field of the `InstallAppPayload`.
    ///
    /// The app can still use existing Cells, i.e. this does not require that
    /// all Cells have DNAs with the same overridden DNA.
    #[serde(default)]
    pub network_seed: Option<NetworkSeed>,

    /// Specify role specific settings or modifiers that will override any settings in
    /// the dna manifets.
    #[serde(default)]
    pub roles_settings: Option<RoleSettingsMap>,

    /// Optional: If app installation fails due to genesis failure, normally the app will be
    /// immediately uninstalled. When this flag is set, the app is left installed with empty cells intact.
    /// This can be useful for using `graft_records_onto_source_chain`, or for diagnostics.
    #[serde(default)]
    pub ignore_genesis_failure: bool,

    /// By default, if an agent key is not specified, the conductor will generate a new one by
    /// deriving a key from the device seed specified in the config. If the device seed is not set,
    /// app installation will fail. If this flag is set, a random key will be created if no device
    /// seed is present. This is a risky decision, because it will mean that if you lose control of
    /// this device, you will not be able to regenerate your agent key from the device seed.
    /// Use only in situations where you know that this is a throwaway key!
    #[serde(default)]
    pub allow_throwaway_random_agent_key: bool,
}

/// Alias
pub type MemproofMap = HashMap<RoleName, MembraneProof>;
/// Alias
pub type ModifiersMap = HashMap<RoleName, DnaModifiersOpt<YamlProperties>>;
/// Alias
pub type ExistingCellsMap = HashMap<RoleName, CellId>;
/// Alias
pub type RoleSettingsMap = HashMap<RoleName, RoleSettings>;

/// Settings for a Role that may be passed on installation of an app
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum RoleSettings {
    /// If the role has the UseExisting strategy defined in the app manifest
    /// the cell id to use needs to be specified here.
    UseExisting(CellId),
    /// Optional settings for a normally provisioned cell
    Provisioned {
        /// When the app being installed has the `allow_deferred_memproofs` manifest flag set,
        /// passing `None` for this field for all roles in the app will allow the app to enter
        /// the "deferred membrane proofs" state, so that memproofs can be provided later via
        /// [`AppRequest::ProvideMemproofs`]. If `Some` is used here, whatever memproofs are
        /// provided will be used, and the app will be installed as normal.
        membrane_proof: Option<MembraneProof>,
        /// Overwrites the dna modifiers from the dna manifest. Only
        /// modifier fields for which `Some(T)` is provided will be overwritten.
        modifiers: Option<DnaModifiersOpt<YamlProperties>>,
    },
}

impl Default for RoleSettings {
    fn default() -> Self {
        Self::Provisioned {
            membrane_proof: None,
            modifiers: None,
        }
    }
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

/// Data about an installed Cell.
#[derive(Clone, Debug, Into, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InstalledCell {
    cell_id: CellId,
    role_name: RoleName,
}

impl InstalledCell {
    /// Constructor
    pub fn new(cell_id: CellId, role_name: RoleName) -> Self {
        Self { cell_id, role_name }
    }

    /// Get the CellId
    pub fn into_id(self) -> CellId {
        self.cell_id
    }

    /// Get the RoleName
    pub fn into_role_name(self) -> RoleName {
        self.role_name
    }

    /// Get the inner data as a tuple
    pub fn into_inner(self) -> (CellId, RoleName) {
        (self.cell_id, self.role_name)
    }

    /// Get the CellId
    pub fn as_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Get the RoleName
    pub fn as_role_name(&self) -> &RoleName {
        &self.role_name
    }
}

/// An app which has been installed.
/// An installed app is merely its collection of "roles", associated with an ID.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    derive_more::Constructor,
    shrinkwraprs::Shrinkwrap,
)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct InstalledApp {
    #[shrinkwrap(main_field)]
    app: InstalledAppCommon,
    /// The status of the installed app
    pub status: AppStatus,
}

impl InstalledApp {
    /// Constructor for freshly installed app
    pub fn new_fresh(app: InstalledAppCommon) -> Self {
        Self {
            app,
            status: AppStatus::Disabled(DisabledAppReason::NeverStarted),
        }
    }

    /// Constructor for freshly installed app
    #[cfg(feature = "test_utils")]
    pub fn new_running(app: InstalledAppCommon) -> Self {
        Self {
            app,
            status: AppStatus::Running,
        }
    }

    /// Return the common app info, as well as a status which encodes the remaining
    /// information
    pub fn into_app_and_status(self) -> (InstalledAppCommon, AppStatus) {
        (self.app, self.status)
    }

    /// Accessor
    pub fn status(&self) -> &AppStatus {
        &self.status
    }

    /// Accessor
    pub fn id(&self) -> &InstalledAppId {
        &self.app.installed_app_id
    }
}

impl automap::AutoMapped for InstalledApp {
    type Key = InstalledAppId;

    fn key(&self) -> &Self::Key {
        &self.app.installed_app_id
    }
}

/// A map from InstalledAppId -> InstalledApp
pub type InstalledAppMap = automap::AutoHashMap<InstalledApp>;

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
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct RunningApp(InstalledAppCommon);

impl RunningApp {
    /// Convert to a StoppedApp with the given reason
    pub fn into_stopped(self, reason: StoppedAppReason) -> StoppedApp {
        StoppedApp {
            app: self.0,
            reason,
        }
    }

    /// Move inner type out
    pub fn into_common(self) -> InstalledAppCommon {
        self.0
    }
}

impl From<RunningApp> for InstalledApp {
    fn from(app: RunningApp) -> Self {
        Self {
            app: app.into_common(),
            status: AppStatus::Running,
        }
    }
}

/// An app which is either Paused or Disabled, i.e. not Running
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, shrinkwraprs::Shrinkwrap,
)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct StoppedApp {
    #[shrinkwrap(main_field)]
    app: InstalledAppCommon,
    reason: StoppedAppReason,
}

impl StoppedApp {
    /// Constructor
    pub fn new_fresh(app: InstalledAppCommon) -> Self {
        Self {
            app,
            reason: StoppedAppReason::Disabled(DisabledAppReason::NeverStarted),
        }
    }

    /// If the app is Stopped, convert into a StoppedApp.
    /// Returns None if app is Running.
    pub fn from_app(app: &InstalledApp) -> Option<Self> {
        StoppedAppReason::from_status(app.status()).map(|reason| Self {
            app: app.as_ref().clone(),
            reason,
        })
    }

    /// Convert to a RunningApp
    pub fn into_active(self) -> RunningApp {
        RunningApp(self.app)
    }

    /// Move inner type out
    pub fn into_common(self) -> InstalledAppCommon {
        self.app
    }
}

impl From<StoppedApp> for InstalledAppCommon {
    fn from(d: StoppedApp) -> Self {
        d.app
    }
}

impl From<StoppedApp> for InstalledApp {
    fn from(d: StoppedApp) -> Self {
        Self {
            app: d.app,
            status: d.reason.into(),
        }
    }
}

/// The common data between apps of any status
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledAppCommon {
    /// The unique identifier for an installed app in this conductor
    pub installed_app_id: InstalledAppId,

    /// The agent key used to install this app.
    pub agent_key: AgentPubKey,

    /// Assignments of DNA roles to cells and their clones, as specified in the AppManifest
    pub role_assignments: IndexMap<RoleName, AppRoleAssignment>,

    /// The manifest used to install the app.
    pub manifest: AppManifest,

    /// The timestamp when this app was installed
    pub installed_at: Timestamp,
}

impl InstalledAppCommon {
    /// Constructor
    pub fn new<S: ToString, I: IntoIterator<Item = (RoleName, AppRoleAssignment)>>(
        installed_app_id: S,
        agent_key: AgentPubKey,
        role_assignments: I,
        manifest: AppManifest,
        installed_at: Timestamp,
    ) -> AppResult<Self> {
        let role_assignments: IndexMap<_, _> = role_assignments.into_iter().collect();
        // ensure no role name contains a clone id delimiter
        if let Some((illegal_role_name, _)) = role_assignments
            .iter()
            .find(|(role_name, _)| role_name.contains(CLONE_ID_DELIMITER))
        {
            return Err(AppError::IllegalRoleName(illegal_role_name.clone()));
        }
        Ok(InstalledAppCommon {
            installed_app_id: installed_app_id.to_string(),
            agent_key,
            role_assignments,
            manifest,
            installed_at,
        })
    }

    /// Accessor
    pub fn id(&self) -> &InstalledAppId {
        &self.installed_app_id
    }

    /// Accessor
    pub fn provisioned_cells(&self) -> impl Iterator<Item = (&RoleName, CellId)> {
        self.role_assignments
            .iter()
            .filter_map(|(role_name, role)| {
                role.provisioned_dna_hash()
                    .map(|d| (role_name, CellId::new(d.clone(), self.agent_key.clone())))
            })
    }

    /// Accessor
    pub fn clone_cells(&self) -> impl Iterator<Item = (&CloneId, CellId)> {
        self.role_assignments
            .iter()
            .flat_map(|app_role_assignment| {
                app_role_assignment
                    .1
                    .as_primary()
                    .into_iter()
                    .flat_map(|p| {
                        p.clones
                            .iter()
                            .map(|(id, d)| (id, CellId::new(d.clone(), self.agent_key.clone())))
                    })
            })
    }

    /// Accessor
    pub fn disabled_clone_cells(&self) -> impl Iterator<Item = (&CloneId, CellId)> {
        self.role_assignments
            .iter()
            .flat_map(|app_role_assignment| {
                app_role_assignment
                    .1
                    .as_primary()
                    .into_iter()
                    .flat_map(|p| {
                        p.disabled_clones
                            .iter()
                            .map(|(id, d)| (id, CellId::new(d.clone(), self.agent_key.clone())))
                    })
            })
    }

    /// Accessor
    pub fn clone_cells_for_role_name(
        &self,
        role_name: &RoleName,
    ) -> Option<impl Iterator<Item = (&CloneId, CellId)>> {
        Some(
            self.role_assignments
                .get(role_name)?
                .as_primary()?
                .clones
                .iter()
                .map(|(id, dna_hash)| (id, CellId::new(dna_hash.clone(), self.agent_key.clone()))),
        )
    }

    /// Accessor
    pub fn disabled_clone_cells_for_role_name(
        &self,
        role_name: &RoleName,
    ) -> Option<impl Iterator<Item = (&CloneId, CellId)>> {
        Some(
            self.role_assignments
                .get(role_name)?
                .as_primary()?
                .disabled_clones
                .iter()
                .map(|(id, dna_hash)| (id, CellId::new(dna_hash.clone(), self.agent_key.clone()))),
        )
    }

    /// Accessor
    pub fn clone_cell_ids(&self) -> impl Iterator<Item = CellId> + '_ {
        self.clone_cells().map(|(_, cell_id)| cell_id)
    }

    /// Accessor
    pub fn disabled_clone_cell_ids(&self) -> impl Iterator<Item = CellId> + '_ {
        self.disabled_clone_cells().map(|(_, cell_id)| cell_id)
    }

    /// Iterator of all cells, both provisioned and cloned.
    // NOTE: as our app state model becomes more nuanced, we need to give careful attention to
    // the definition of this function, since this represents all cells in use by the conductor.
    // Any cell which exists and is not returned by this function is fair game for purging
    // during app installation. See [`Conductor::remove_dangling_cells`].
    pub fn all_cells(&self) -> impl Iterator<Item = CellId> + '_ {
        self.provisioned_cells()
            .map(|(_, c)| c)
            .chain(self.clone_cell_ids())
            .chain(self.disabled_clone_cell_ids())
    }

    /// Iterator of all running cells, both provisioned and cloned.
    /// Provisioned cells will always be running if the app is running,
    /// but some cloned cells may be disabled and will not be returned.
    pub fn all_enabled_cells(&self) -> impl Iterator<Item = CellId> + '_ {
        self.provisioned_cells()
            .map(|(_, c)| c)
            .chain(self.clone_cell_ids())
    }

    /// Iterator of all "required" cells, meaning Cells which must be running
    /// for this App to be able to run.
    ///
    /// Currently this is simply all provisioned cells, but this concept may
    /// become more nuanced in the future.
    pub fn required_cells(&self) -> impl Iterator<Item = CellId> + '_ {
        self.provisioned_cells().map(|(_, c)| c)
    }

    /// Accessor for particular role
    pub fn role(&self, role_name: &RoleName) -> AppResult<&AppRoleAssignment> {
        self.role_assignments
            .get(role_name)
            .ok_or_else(|| AppError::RoleNameMissing(role_name.clone()))
    }

    /// If the role is primary, i.e. of variant [`AppRoleassignment::Primary`], return it
    /// as [`AppRolePrimary`]. If the role is not primary, return Err.
    pub fn primary_role(&self, role_name: &RoleName) -> AppResult<&AppRolePrimary> {
        let app_id = self.installed_app_id.clone();
        self.role(role_name)?
            .as_primary()
            .ok_or_else(|| AppError::NonPrimaryCell(app_id, role_name.clone()))
    }

    fn role_mut(&mut self, role_name: &RoleName) -> AppResult<&mut AppRoleAssignment> {
        self.role_assignments
            .get_mut(role_name)
            .ok_or_else(|| AppError::RoleNameMissing(role_name.clone()))
    }

    fn primary_role_mut(&mut self, role_name: &RoleName) -> AppResult<&mut AppRolePrimary> {
        let app_id = self.installed_app_id.clone();
        self.role_mut(role_name)?
            .as_primary_mut()
            .ok_or_else(|| AppError::NonPrimaryCell(app_id, role_name.clone()))
    }

    /// Accessor
    pub fn roles(&self) -> &IndexMap<RoleName, AppRoleAssignment> {
        &self.role_assignments
    }

    /// Accessor
    pub fn primary_roles(&self) -> impl Iterator<Item = (&RoleName, &AppRolePrimary)> {
        self.role_assignments
            .iter()
            .filter_map(|(name, role)| Some((name, role.as_primary()?)))
    }

    /// Add a clone cell.
    pub fn add_clone(&mut self, role_name: &RoleName, dna_hash: &DnaHash) -> AppResult<CloneId> {
        let app_role_assignment = self.primary_role_mut(role_name)?;

        if app_role_assignment.is_clone_limit_reached() {
            return Err(AppError::CloneLimitExceeded(
                app_role_assignment.clone_limit,
                app_role_assignment.clone(),
            ));
        }
        let clone_id = CloneId::new(role_name, app_role_assignment.next_clone_index);
        if app_role_assignment.clones.contains_key(&clone_id) {
            return Err(AppError::DuplicateCloneIds(clone_id));
        }

        // add clone
        app_role_assignment
            .clones
            .insert(clone_id.clone(), dna_hash.clone());
        // increment next clone index
        app_role_assignment.next_clone_index += 1;
        Ok(clone_id)
    }

    /// Get a clone cell id from its clone id.
    pub fn get_clone_dna_hash(&self, clone_cell_id: &CloneCellId) -> AppResult<DnaHash> {
        let cell_id = match clone_cell_id {
            CloneCellId::DnaHash(dna_hash) => dna_hash,
            CloneCellId::CloneId(clone_id) => self
                .primary_role(&clone_id.as_base_role_name())?
                .clones
                .get(clone_id)
                .ok_or_else(|| {
                    AppError::CloneCellNotFound(CloneCellId::CloneId(clone_id.clone()))
                })?,
        };
        Ok(cell_id.clone())
    }

    /// Get the clone id from either clone or cell id.
    pub fn get_clone_id(&self, clone_cell_id: &CloneCellId) -> AppResult<CloneId> {
        let clone_id = match clone_cell_id {
            CloneCellId::CloneId(id) => id,
            CloneCellId::DnaHash(id) => {
                self.clone_cells()
                    .find(|(_, cell_id)| cell_id.dna_hash() == id)
                    .ok_or_else(|| AppError::CloneCellNotFound(CloneCellId::DnaHash(id.clone())))?
                    .0
            }
        };
        Ok(clone_id.clone())
    }

    /// Get the clone id from either clone or cell id.
    pub fn get_disabled_clone_id(&self, clone_cell_id: &CloneCellId) -> AppResult<CloneId> {
        let clone_id = match clone_cell_id {
            CloneCellId::CloneId(id) => id.clone(),
            CloneCellId::DnaHash(id) => self
                .role_assignments
                .iter()
                .flat_map(|(_, role_assignment)| {
                    role_assignment
                        .as_primary()
                        .into_iter()
                        .flat_map(|r| r.disabled_clones.iter())
                })
                .find(|(_, cell_id)| *cell_id == id)
                .ok_or_else(|| AppError::CloneCellNotFound(CloneCellId::DnaHash(id.clone())))?
                .0
                .clone(),
        };
        Ok(clone_id)
    }

    /// Disable a clone cell.
    ///
    /// Removes the cell from the list of clones, so it is not accessible any
    /// longer. If the cell is already disabled, do nothing and return Ok.
    pub fn disable_clone_cell(&mut self, clone_id: &CloneId) -> AppResult<()> {
        let app_role_assignment = self.primary_role_mut(&clone_id.as_base_role_name())?;
        // remove clone from role's clones map
        match app_role_assignment.clones.remove(clone_id) {
            None => {
                if app_role_assignment.disabled_clones.contains_key(clone_id) {
                    Ok(())
                } else {
                    Err(AppError::CloneCellNotFound(CloneCellId::CloneId(
                        clone_id.to_owned(),
                    )))
                }
            }
            Some(cell_id) => {
                // insert clone into disabled clones map
                let insert_result = app_role_assignment
                    .disabled_clones
                    .insert(clone_id.to_owned(), cell_id);
                assert!(
                    insert_result.is_none(),
                    "disable: clone cell is already disabled"
                );
                Ok(())
            }
        }
    }

    /// Enable a disabled clone cell.
    ///
    /// The clone cell is added back to the list of clones and can be accessed
    /// again. If the cell is already enabled, do nothing and return Ok.
    ///
    /// # Returns
    /// The enabled clone cell.
    pub fn enable_clone_cell(&mut self, clone_id: &CloneId) -> AppResult<InstalledCell> {
        let app_role_assignment = self.primary_role_mut(&clone_id.as_base_role_name())?;
        // remove clone from disabled clones map
        match app_role_assignment.disabled_clones.remove(clone_id) {
            None => app_role_assignment
                .clones
                .get(clone_id)
                .cloned()
                .map(|dna_hash| {
                    Ok(InstalledCell {
                        role_name: clone_id.as_app_role_name().to_owned(),
                        cell_id: CellId::new(dna_hash, self.agent_key.clone()),
                    })
                })
                .unwrap_or_else(|| {
                    Err(AppError::CloneCellNotFound(CloneCellId::CloneId(
                        clone_id.to_owned(),
                    )))
                }),
            Some(dna_hash) => {
                // insert clone back into role's clones map
                let insert_result = app_role_assignment
                    .clones
                    .insert(clone_id.to_owned(), dna_hash.clone());
                assert!(
                    insert_result.is_none(),
                    "enable: clone cell already enabled"
                );
                Ok(InstalledCell {
                    role_name: clone_id.as_app_role_name().to_owned(),
                    cell_id: CellId::new(dna_hash, self.agent_key.clone()),
                })
            }
        }
    }

    /// Delete a disabled clone cell.
    pub fn delete_clone_cell(&mut self, clone_id: &CloneId) -> AppResult<()> {
        let app_role_assignment = self.primary_role_mut(&clone_id.as_base_role_name())?;
        app_role_assignment
            .disabled_clones
            .remove(clone_id)
            .map(|_| ())
            .ok_or_else(|| {
                if app_role_assignment.clones.contains_key(clone_id) {
                    AppError::CloneCellMustBeDisabledBeforeDeleting(CloneCellId::CloneId(
                        clone_id.to_owned(),
                    ))
                } else {
                    AppError::CloneCellNotFound(CloneCellId::CloneId(clone_id.to_owned()))
                }
            })
    }

    /// Accessor
    pub fn agent_key(&self) -> &AgentPubKey {
        &self.agent_key
    }

    /// Constructor for apps not using a manifest.
    /// Allows for cloning up to 256 times and implies immediate provisioning.
    #[cfg(feature = "test_utils")]
    pub fn new_legacy<S: ToString, I: IntoIterator<Item = InstalledCell>>(
        installed_app_id: S,
        installed_cells: I,
    ) -> AppResult<Self> {
        use itertools::Itertools;

        let installed_app_id = installed_app_id.to_string();
        let installed_cells: Vec<_> = installed_cells.into_iter().collect();

        // Get the agent key of the first cell
        // NB: currently this has no significance.
        let agent_key = installed_cells
            .first()
            .expect("Can't create app with 0 cells")
            .cell_id
            .agent_pubkey()
            .to_owned();

        // ensure all cells use the same agent key
        if installed_cells
            .iter()
            .any(|c| *c.cell_id.agent_pubkey() != agent_key)
        {
            panic!(
                "All cells in an app must use the same agent key. Cell data: {:#?}",
                installed_cells
            );
        }

        // ensure all cells use the same agent key
        let duplicates: Vec<RoleName> = installed_cells
            .iter()
            .map(|c| c.role_name.to_owned())
            .counts()
            .into_iter()
            .filter_map(|(role_name, count)| if count > 1 { Some(role_name) } else { None })
            .collect();
        if !duplicates.is_empty() {
            return Err(AppError::DuplicateRoleNames(installed_app_id, duplicates));
        }

        let manifest = AppManifest::from_legacy(installed_cells.clone().into_iter());

        let role_assignments = installed_cells
            .into_iter()
            .map(|InstalledCell { role_name, cell_id }| {
                let role = AppRolePrimary {
                    base_dna_hash: cell_id.dna_hash().clone(),
                    is_provisioned: true,
                    clones: HashMap::new(),
                    clone_limit: 256,
                    next_clone_index: 0,
                    disabled_clones: HashMap::new(),
                };
                (role_name, role.into())
            })
            .collect();

        Ok(Self {
            installed_app_id,
            agent_key,
            role_assignments,
            manifest,
            installed_at: Timestamp::now(),
        })
    }

    // pub fn dependencies(&self) -> Vec<

    /// Return the manifest if available
    pub fn manifest(&self) -> &AppManifest {
        &self.manifest
    }

    /// Return the list of role assignments
    pub fn role_assignments(&self) -> &IndexMap<RoleName, AppRoleAssignment> {
        &self.role_assignments
    }

    /// Accessor
    pub fn installed_at(&self) -> &Timestamp {
        &self.installed_at
    }
}

/// The status of an installed app.
///
/// App Status is a combination of two pieces of independent state:
/// - Enabled/Disabled, which is a designation set by the user via the conductor admin interface.
/// - Running/Stopped, which is a fact about the reality of the app in the course of its operation.
///
/// The combinations of these basic states give rise to the unified App Status.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum AppStatus {
    /// The app is enabled and running normally.
    Running,

    /// Enabled, but stopped due to some recoverable problem.
    /// The app "hopes" to be Running again as soon as possible.
    /// Holochain may restart the app automatically if it can. It may also be
    /// restarted manually via the `StartApp` admin method.
    /// Paused apps will be automatically set to Running when the conductor restarts.
    Paused(PausedAppReason),

    /// Disabled and stopped, either manually by the user, or automatically due
    /// to an unrecoverable error. App must be Enabled before running again,
    /// and will not restart automaticaly on conductor reboot.
    Disabled(DisabledAppReason),

    /// The app is installed, but genesis has not completed due to use of
    /// [`MembraneProofProvisioning::Deferred`]
    AwaitingMemproofs,
}

/// The AppStatus without the reasons.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum AppStatusKind {
    Running,
    Paused,
    Disabled,
    AwaitingMemproofs,
}

impl From<AppStatus> for AppStatusKind {
    fn from(status: AppStatus) -> Self {
        match status {
            AppStatus::Running => Self::Running,
            AppStatus::Paused(_) => Self::Paused,
            AppStatus::Disabled(_) => Self::Disabled,
            AppStatus::AwaitingMemproofs => Self::AwaitingMemproofs,
        }
    }
}

/// Represents a state transition operation from one state to another
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppStatusTransition {
    /// Attempt to unpause a Paused app
    Start,
    /// Attempt to pause a Running app
    Pause(PausedAppReason),
    /// Gets an app running no matter what
    Enable,
    /// Disables an app, no matter what
    Disable(DisabledAppReason),
}

impl AppStatus {
    /// Does this status correspond to an Enabled state?
    /// If false, this indicates a Disabled state.
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Running | Self::Paused(_))
    }

    /// Does this status correspond to a Running state?
    /// If false, this indicates a Stopped state.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Does this status correspond to a Paused state?
    pub fn is_paused(&self) -> bool {
        matches!(self, Self::Paused(_))
    }

    /// Transition a status from one state to another.
    /// If None, the transition was not valid, and the status did not change.
    pub fn transition(&mut self, transition: AppStatusTransition) -> AppStatusFx {
        use AppStatus::*;
        use AppStatusFx::*;
        use AppStatusTransition::*;
        match (&self, transition) {
            (Running, Pause(reason)) => Some((Paused(reason), SpinDown)),
            (Running, Disable(reason)) => Some((Disabled(reason), SpinDown)),
            (Running, Start) | (Running, Enable) => None,

            (Paused(_), Start) => Some((Running, SpinUp)),
            (Paused(_), Enable) => Some((Running, SpinUp)),
            (Paused(_), Disable(reason)) => Some((Disabled(reason), SpinDown)),
            (Paused(_), Pause(_)) => None,

            (Disabled(_), Enable) => Some((Running, SpinUp)),
            (Disabled(_), Pause(_)) | (Disabled(_), Disable(_)) | (Disabled(_), Start) => None,

            (AwaitingMemproofs, Enable | Start) => Some((
                AwaitingMemproofs,
                Error("Cannot enable an app which is AwaitingMemproofs".to_string()),
            )),
            (AwaitingMemproofs, _) => None,
        }
        .map(|(new_status, delta)| {
            *self = new_status;
            delta
        })
        .unwrap_or(NoChange)
    }
}

/// A declaration of the side effects of a particular AppStatusTransition.
///
/// Two values of this type may also be combined into one,
/// to capture the overall effect of a series of transitions.
///
/// The intent of this type is to make sure that any operation which causes an
/// app state transition is followed up with a call to process_app_status_fx
/// in order to reconcile the cell state with the new app state.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "be sure to run this value through `process_app_status_fx` to handle any transition effects"]
pub enum AppStatusFx {
    /// The transition did not result in any change to CellState.
    NoChange,
    /// The transition may cause some Cells to be removed.
    SpinDown,
    /// The transition may cause some Cells to be added (fallibly).
    SpinUp,
    /// The transition may cause some Cells to be removed and some to be (fallibly) added.
    Both,
    /// The transition was invalid and should produce an error.
    Error(String),
}

impl Default for AppStatusFx {
    fn default() -> Self {
        Self::NoChange
    }
}

impl AppStatusFx {
    /// Combine two effects into one. Think "monoidal append", if that helps.
    pub fn combine(self, other: Self) -> Self {
        use AppStatusFx::*;
        match (self, other) {
            (NoChange, a) | (a, NoChange) => a,
            (SpinDown, SpinDown) => SpinDown,
            (SpinUp, SpinUp) => SpinUp,
            (Both, _) | (_, Both) => Both,
            (SpinDown, SpinUp) | (SpinUp, SpinDown) => Both,
            (Error(err1), Error(err2)) => Error(format!("{err1}. {err2}")),
            (Error(err), _) | (_, Error(err)) => Error(err),
        }
    }
}

/// The various reasons for why an App is not in the Running state.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
    derive_more::From,
)]
#[serde(rename_all = "snake_case")]
pub enum StoppedAppReason {
    /// Same meaning as [`AppStatus::Paused`].
    Paused(PausedAppReason),

    /// Same meaning as [`AppStatus::Disabled`].
    Disabled(DisabledAppReason),

    /// Same meaning as [`AppStatus::AwaitingMemProofs`].
    AwaitingMemproofs,
}

impl StoppedAppReason {
    /// Convert a status into a StoppedAppReason.
    /// If the status is Running, returns None.
    pub fn from_status(status: &AppStatus) -> Option<Self> {
        match status {
            AppStatus::Paused(reason) => Some(Self::Paused(reason.clone())),
            AppStatus::Disabled(reason) => Some(Self::Disabled(reason.clone())),
            AppStatus::AwaitingMemproofs => Some(Self::AwaitingMemproofs),
            AppStatus::Running => None,
        }
    }
}

impl From<StoppedAppReason> for AppStatus {
    fn from(reason: StoppedAppReason) -> Self {
        match reason {
            StoppedAppReason::Paused(reason) => Self::Paused(reason),
            StoppedAppReason::Disabled(reason) => Self::Disabled(reason),
            StoppedAppReason::AwaitingMemproofs => Self::AwaitingMemproofs,
        }
    }
}

/// The reason for an app being in a Paused state.
/// NB: there is no way to manually pause an app.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum PausedAppReason {
    /// The pause was due to a RECOVERABLE error
    Error(String),
}

/// The reason for an app being in a Disabled state.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum DisabledAppReason {
    /// The app is freshly installed, and never started
    NeverStarted,
    /// The app is fully installed and deferred memproofs have been provided by the UI,
    /// but the app has not been enabled.
    /// The app can be enabled via the app interface in this state, which is why this is
    /// separate from other disabled states.
    NotStartedAfterProvidingMemproofs,
    /// The disabling was done manually by the user (via admin interface)
    User,
    /// Disabling app in order to revoke its agent key and render all chains read-only.
    DeletingAgentKey,
    /// The disabling was due to an UNRECOVERABLE error
    Error(String),
}

/// App "roles" correspond to cell entries in the AppManifest.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
pub enum AppRoleAssignment {
    /// A "primary" role assignment indicates that this app "owns" this cell.
    /// The cell was created during app installation, and corresponds to the
    /// Create and CloneOnly CellProvisioning strategies.
    Primary(AppRolePrimary),
    /// A "dependency" role assignment indicates that the cell is owned by some other app,
    /// and this cell depends upon it.
    Dependency(AppRoleDependency),
}

impl AppRoleAssignment {
    /// Use the Primary variant
    pub fn as_primary(&self) -> Option<&AppRolePrimary> {
        match self {
            Self::Primary(p) => Some(p),
            Self::Dependency(_) => None,
        }
    }

    /// Use the Primary variant
    pub fn as_primary_mut(&mut self) -> Option<&mut AppRolePrimary> {
        match self {
            Self::Primary(p) => Some(p),
            Self::Dependency(_) => None,
        }
    }

    /// Accessor
    pub fn provisioned_dna_hash(&self) -> Option<&DnaHash> {
        match self {
            Self::Primary(p) => p.provisioned_dna_hash(),
            Self::Dependency(_) => None,
        }
    }

    // /// Accessor
    // pub fn cell_id(&self) -> &CellId {
    //     match self {
    //         Self::Primary(p) => p.dna_hash(),
    //         Self::Dependency(d) => &d.cell_id,
    //     }
    // }
}

/// An app role whose cell(s) were created by the installation of this app.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppRolePrimary {
    /// The Id of the Cell which will be provisioned for this role.
    /// This also identifies the basis for cloned DNAs, and this is how the
    /// Agent is determined for clones (always the same as the provisioned cell).
    pub base_dna_hash: DnaHash,
    /// Records whether the base cell has actually been provisioned or not.
    /// If true, then `base_dna_hash` refers to an actual existing Cell with
    /// that DNA hash.
    /// If false, then `base_dna_hash` is referring to a future cell which will
    /// be created with that DNA hash.
    pub is_provisioned: bool,
    /// The number of allowed clone cells.
    pub clone_limit: u32,

    /// The index of the next clone cell to be created.
    pub next_clone_index: u32,

    /// Cells which were cloned at runtime. The length cannot grow beyond
    /// `clone_limit`.
    pub clones: HashMap<CloneId, DnaHash>,
    /// Clone cells that have been disabled. These cells cannot be called
    /// any longer and are not returned as part of the app info either.
    /// Disabled clone cells can be deleted through the Admin API.
    pub disabled_clones: HashMap<CloneId, DnaHash>,
}

impl AppRolePrimary {
    /// Constructor. List of clones always starts empty.
    pub fn new(base_dna_hash: DnaHash, is_provisioned: bool, clone_limit: u32) -> Self {
        Self {
            base_dna_hash,
            is_provisioned,
            clone_limit,
            clones: HashMap::new(),
            next_clone_index: 0,
            disabled_clones: HashMap::new(),
        }
    }

    /// Accessor
    pub fn dna_hash(&self) -> &DnaHash {
        &self.base_dna_hash
    }

    /// Accessor
    pub fn provisioned_dna_hash(&self) -> Option<&DnaHash> {
        if self.is_provisioned {
            Some(&self.base_dna_hash)
        } else {
            None
        }
    }

    /// Accessor
    pub fn clone_ids(&self) -> impl Iterator<Item = &CloneId> {
        self.clones.keys()
    }

    /// Accessor
    pub fn clone_limit(&self) -> u32 {
        self.clone_limit
    }

    /// Accessor
    pub fn is_clone_limit_reached(&self) -> bool {
        self.clones.len() as u32 == self.clone_limit
    }
}

/// An app role which is filled by a cell created by another app's primary role.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppRoleDependency {
    /// The cell which is depended upon.
    pub cell_id: CellId,
    /// Whether this dependency is protected: if true, the dependent cell's app
    /// cannot be uninstalled without first uninstalling this app (except by
    /// using the `force` flag of UninstallApp).
    pub protected: bool,
}

#[cfg(test)]
mod tests {
    use super::RunningApp;
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use arbitrary::Arbitrary;
    use std::collections::HashSet;

    #[test]
    fn illegal_role_name_is_rejected() {
        let mut u = unstructured_noise();
        let result = InstalledAppCommon::new(
            "test_app",
            fixt!(AgentPubKey),
            vec![(
                CLONE_ID_DELIMITER.into(),
                AppRolePrimary::new(fixt!(DnaHash), false, 0).into(),
            )],
            AppManifest::arbitrary(&mut u).unwrap(),
            Timestamp::now(),
        );
        assert!(result.is_err())
    }

    #[test]
    fn clone_management() {
        let base_dna_hash = fixt!(DnaHash);
        let new_clone = || fixt!(DnaHash);
        let clone_limit = 3;
        let role1 = AppRolePrimary::new(base_dna_hash, false, clone_limit).into();
        let agent = fixt!(AgentPubKey);
        let role_name: RoleName = "role_name".into();
        let manifest = AppManifest::arbitrary(&mut unstructured_noise()).unwrap();
        let mut app: RunningApp = InstalledAppCommon::new(
            "app",
            agent.clone(),
            vec![(role_name.clone(), role1)],
            manifest,
            Timestamp::now(),
        )
        .unwrap()
        .into();

        // Can add clones up to the limit
        let clones: Vec<_> = vec![new_clone(), new_clone(), new_clone()];
        let clone_id_0 = app.add_clone(&role_name, &clones[0]).unwrap();
        let clone_id_1 = app.add_clone(&role_name, &clones[1]).unwrap();
        let clone_id_2 = app.add_clone(&role_name, &clones[2]).unwrap();

        assert_eq!(clone_id_0, CloneId::new(&role_name, 0));
        assert_eq!(clone_id_1, CloneId::new(&role_name, 1));
        assert_eq!(clone_id_2, CloneId::new(&role_name, 2));

        assert_eq!(
            app.clone_cell_ids()
                .map(|id| id.dna_hash().clone())
                .collect::<HashSet<_>>(),
            clones.clone().into_iter().collect::<HashSet<_>>()
        );
        assert_eq!(app.clone_cells().count(), 3);

        // Adding the same clone twice should return an error
        let result_add_clone_twice = app.add_clone(&role_name, &clones[0]);
        assert!(result_add_clone_twice.is_err());

        // Adding a clone beyond the clone_limit is an error
        matches::assert_matches!(
            app.add_clone(&role_name, &new_clone()),
            Err(AppError::CloneLimitExceeded(3, _))
        );

        // Disable a clone cell
        app.disable_clone_cell(&clone_id_0).unwrap();
        // Assert it is moved to disabled clone cells
        assert!(!app
            .clone_cells()
            .any(|(clone_id, _)| *clone_id == clone_id_0));
        assert_eq!(app.clone_cells().count(), 2);
        assert!(app
            .disabled_clone_cells()
            .any(|(clone_id, _)| *clone_id == clone_id_0));

        // Enable a disabled clone cell
        let enabled_cell = app.enable_clone_cell(&clone_id_0).unwrap();
        assert_eq!(
            enabled_cell.role_name,
            clone_id_0.as_app_role_name().to_owned()
        );

        // Enabling an already enabled cell does nothing.
        let enabled_cell_2 = app.enable_clone_cell(&clone_id_0).unwrap();
        assert_eq!(enabled_cell_2, enabled_cell);

        // Assert it is accessible from the app again
        assert!(app
            .clone_cells()
            .any(|(clone_id, _)| *clone_id == clone_id_0));
        assert_eq!(
            app.clone_cell_ids()
                .map(|id| id.dna_hash().clone())
                .collect::<HashSet<_>>(),
            clones.clone().into_iter().collect::<HashSet<_>>()
        );
        assert_eq!(app.clone_cells().count(), 3);

        // Disable and delete a clone cell
        app.disable_clone_cell(&clone_id_0).unwrap();
        // Disabling is also idempotent
        app.disable_clone_cell(&clone_id_0).unwrap();

        app.delete_clone_cell(&clone_id_0).unwrap();
        // Assert the deleted cell cannot be enabled
        assert!(app.enable_clone_cell(&clone_id_0).is_err());
    }
}
