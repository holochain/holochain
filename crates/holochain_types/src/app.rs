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
#[deprecated = "Remove when InstallApp goes away; use AppRoleId instead"]
pub type CellNick = String;

/// Identifier for an Approle
pub type AppRoleId = String;

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
    /// The Role ID under which to create this clone
    /// (needed to track cloning permissions and `clone_count`)
    pub role_id: AppRoleId,
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
    // TODO: rename to role_id
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
    #[deprecated = "should only be constructable through conversions from other types"]
    pub fn new(app: InstalledAppCommon, reason: StoppedAppReason) -> Self {
        Self { app, reason }
    }

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
    installed_app_id: InstalledAppId,
    /// The agent key used to install this app. Currently this is meaningless,
    /// but I'm leaving it here as a placeholder in case we ever want it to
    /// have formal significance.
    _agent_key: AgentPubKey,
    /// The "roles" as specified in the AppManifest
    roles: HashMap<CellNick, AppRole>,
}

impl InstalledAppCommon {
    /// Constructor
    pub fn new<S: ToString, I: IntoIterator<Item = (AppRoleId, AppRole)>>(
        installed_app_id: S,
        _agent_key: AgentPubKey,
        roles: I,
    ) -> Self {
        InstalledAppCommon {
            installed_app_id: installed_app_id.to_string(),
            _agent_key,
            roles: roles.into_iter().collect(),
        }
    }

    /// Accessor
    pub fn id(&self) -> &InstalledAppId {
        &self.installed_app_id
    }

    /// Accessor
    pub fn provisioned_cells(&self) -> impl Iterator<Item = (&AppRoleId, &CellId)> {
        self.roles
            .iter()
            .filter_map(|(nick, role)| role.provisioned_cell().map(|c| (nick, c)))
    }

    /// Accessor
    pub fn into_provisioned_cells(self) -> impl Iterator<Item = (AppRoleId, CellId)> {
        self.roles
            .into_iter()
            .filter_map(|(nick, role)| role.into_provisioned_cell().map(|c| (nick, c)))
    }

    /// Accessor
    pub fn cloned_cells(&self) -> impl Iterator<Item = &CellId> {
        self.roles.iter().map(|(_, role)| &role.clones).flatten()
    }

    /// Iterator of all cells, both provisioned and cloned
    pub fn all_cells(&self) -> impl Iterator<Item = &CellId> {
        self.provisioned_cells()
            .map(|(_, c)| c)
            .chain(self.cloned_cells())
    }

    /// Iterator of all "required" cells, meaning Cells which must be running
    /// for this App to be able to run. The notion of "required cells" is not
    /// yet solidified, so for now this placeholder equates to "all cells".
    pub fn required_cells(&self) -> impl Iterator<Item = &CellId> {
        self.all_cells()
    }

    /// Accessor for particular role
    pub fn role(&self, role_id: &AppRoleId) -> AppResult<&AppRole> {
        self.roles
            .get(role_id)
            .ok_or_else(|| AppError::AppRoleIdMissing(role_id.clone()))
    }

    fn role_mut(&mut self, role_id: &AppRoleId) -> AppResult<&mut AppRole> {
        self.roles
            .get_mut(role_id)
            .ok_or_else(|| AppError::AppRoleIdMissing(role_id.clone()))
    }

    /// Accessor
    pub fn roles(&self) -> &HashMap<AppRoleId, AppRole> {
        &self.roles
    }

    /// Add a cloned cell
    pub fn add_clone(&mut self, role_id: &AppRoleId, cell_id: CellId) -> AppResult<()> {
        let role = self.role_mut(role_id)?;
        assert_eq!(
            cell_id.agent_pubkey(),
            role.agent_key(),
            "A clone cell must use the same agent key as the role it is added to"
        );
        if role.clones.len() as u32 >= role.clone_limit {
            return Err(AppError::CloneLimitExceeded(role.clone_limit, role.clone()));
        }
        let _ = role.clones.insert(cell_id);
        Ok(())
    }

    /// Remove a cloned cell
    pub fn remove_clone(&mut self, role_id: &AppRoleId, cell_id: &CellId) -> AppResult<bool> {
        let role = self.role_mut(role_id)?;
        Ok(role.clones.remove(cell_id))
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
            return Err(AppError::DuplicateAppRoleIds(installed_app_id, duplicates));
        }

        let roles = installed_cells
            .into_iter()
            .map(|InstalledCell { cell_nick, cell_id }| {
                let role = AppRole {
                    base_cell_id: cell_id,
                    is_provisioned: true,
                    clones: HashSet::new(),
                    clone_limit: 0,
                };
                (cell_nick, role)
            })
            .collect();
        Ok(Self {
            installed_app_id,
            _agent_key,
            roles,
        })
    }
}

/// The status of an installed app.
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
}

/// The AppStatus without the reasons.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum AppStatusKind {
    Running,
    Paused,
    Disabled,
}

impl From<AppStatus> for AppStatusKind {
    fn from(status: AppStatus) -> Self {
        match status {
            AppStatus::Running => Self::Running,
            AppStatus::Paused(_) => Self::Paused,
            AppStatus::Disabled(_) => Self::Disabled,
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
    /// Same meaning as [`InstalledAppStatus::Paused`].
    Paused(PausedAppReason),

    /// Same meaning as [`InstalledAppStatus::Disabled`].
    Disabled(DisabledAppReason),
}

impl StoppedAppReason {
    /// Convert a status into a StoppedAppReason.
    /// If the status is Running, returns None.
    pub fn from_status(status: &AppStatus) -> Option<Self> {
        match status {
            AppStatus::Paused(reason) => Some(Self::Paused(reason.clone())),
            AppStatus::Disabled(reason) => Some(Self::Disabled(reason.clone())),
            AppStatus::Running => None,
        }
    }
}

impl From<StoppedAppReason> for AppStatus {
    fn from(reason: StoppedAppReason) -> Self {
        match reason {
            StoppedAppReason::Paused(reason) => Self::Paused(reason),
            StoppedAppReason::Disabled(reason) => Self::Disabled(reason),
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
    /// The disabling was done manually by the user (via admin interface)
    User,
    /// The disabling was due to an UNRECOVERABLE error
    Error(String),
}

/// Cell "roles" correspond to cell entries in the AppManifest.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppRole {
    /// The Id of the Cell which will be provisioned for this role.
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

impl AppRole {
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
    use super::{AppRole, RunningApp};
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use std::collections::HashSet;

    #[test]
    fn clone_management() {
        let base_cell_id = fixt!(CellId);
        let agent = base_cell_id.agent_pubkey().clone();
        let new_clone = || CellId::new(fixt!(DnaHash), agent.clone());
        let role1 = AppRole::new(base_cell_id, false, 3);
        let agent = fixt!(AgentPubKey);
        let role_id: AppRoleId = "role_id".into();
        let mut app: RunningApp =
            InstalledAppCommon::new("app", agent.clone(), vec![(role_id.clone(), role1)]).into();

        // Can add clones up to the limit
        let clones: Vec<_> = vec![new_clone(), new_clone(), new_clone()];
        app.add_clone(&role_id, clones[0].clone()).unwrap();
        app.add_clone(&role_id, clones[1].clone()).unwrap();
        app.add_clone(&role_id, clones[2].clone()).unwrap();

        // Adding a clone beyond the clone_limit is an error
        matches::assert_matches!(
            app.add_clone(&role_id, new_clone()),
            Err(AppError::CloneLimitExceeded(3, _))
        );

        assert_eq!(
            app.cloned_cells().collect::<HashSet<_>>(),
            maplit::hashset! { &clones[0], &clones[1], &clones[2] }
        );

        assert_eq!(app.remove_clone(&role_id, &clones[1]).unwrap(), true);
        assert_eq!(app.remove_clone(&role_id, &clones[1]).unwrap(), false);

        assert_eq!(
            app.cloned_cells().collect::<HashSet<_>>(),
            maplit::hashset! { &clones[0], &clones[2] }
        );

        // Adding the same clone twice should probably be a panic, but if this
        // line is still here, I never got around to making it panic...
        app.add_clone(&role_id, clones[0].clone()).unwrap();

        assert_eq!(app.cloned_cells().count(), 2);

        assert_eq!(
            app.cloned_cells().collect::<HashSet<_>>(),
            app.all_cells().collect::<HashSet<_>>()
        );
    }
}
