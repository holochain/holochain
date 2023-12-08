use std::collections::HashMap;

use itertools::Itertools;

use holochain_dna_types::prelude::*;

use crate::{
    error::{AppError, AppResult},
    *,
};

pub use holochain_cell::organ::*;

/// The unique identifier for an installed app in this conductor
pub type AppId = String;

/// Data about an installed Cell.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InstalledCell {
    pub cell_id: CellId,
    pub role_name: RoleName,
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
    derive_more::Deref,
    derive_more::DerefMut,
)]
pub struct InstalledApp {
    #[deref]
    #[deref_mut]
    app: InstalledAppCommon,
    /// The status of the installed app
    pub status: OrganStatus,
}

impl InstalledApp {
    /// Constructor for freshly installed app
    pub fn new_fresh(app: InstalledAppCommon) -> Self {
        Self {
            app,
            status: OrganStatus::Disabled(DisabledOrganReason::NeverStarted),
        }
    }

    /// Constructor for freshly installed app
    #[cfg(feature = "test_utils")]
    pub fn new_running(app: InstalledAppCommon) -> Self {
        Self {
            app,
            status: OrganStatus::Running,
        }
    }

    /// Return the common app info, as well as a status which encodes the remaining
    /// information
    pub fn into_app_and_status(self) -> (InstalledAppCommon, OrganStatus) {
        (self.app, self.status)
    }

    /// Accessor
    pub fn status(&self) -> &OrganStatus {
        &self.status
    }

    /// Accessor
    pub fn id(&self) -> &AppId {
        &self.app.installed_app_id
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
    derive_more::Deref,
    derive_more::DerefMut,
)]
pub struct RunningApp(InstalledAppCommon);

impl RunningApp {
    /// Convert to a StoppedApp with the given reason
    pub fn into_stopped(self, reason: StoppedOrganReason) -> StoppedApp {
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
            status: OrganStatus::Running,
        }
    }
}

/// An app which is either Paused or Disabled, i.e. not Running
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    derive_more::Deref,
    derive_more::DerefMut,
)]
pub struct StoppedApp {
    #[deref]
    #[deref_mut]
    app: InstalledAppCommon,
    reason: StoppedOrganReason,
}

impl StoppedApp {
    /// Constructor
    #[deprecated = "should only be constructable through conversions from other types"]
    pub fn new(app: InstalledAppCommon, reason: StoppedOrganReason) -> Self {
        Self { app, reason }
    }

    /// Constructor
    pub fn new_fresh(app: InstalledAppCommon) -> Self {
        Self {
            app,
            reason: StoppedOrganReason::Disabled(DisabledOrganReason::NeverStarted),
        }
    }

    /// If the app is Stopped, convert into a StoppedApp.
    /// Returns None if app is Running.
    pub fn from_app(app: &InstalledApp) -> Option<Self> {
        StoppedOrganReason::from_status(app.status()).map(|reason| Self {
            app: (**app).clone(),
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
    installed_app_id: AppId,
    /// The agent key used to install this app.
    agent_key: AgentPubKey,
    /// Assignments of DNA roles to cells and their clones, as specified in the AppManifest
    role_assignments: HashMap<RoleName, AppRoleAssignment>,
    /// The manifest used to install the app.
    manifest: AppManifest,
}

impl InstalledAppCommon {
    /// Constructor
    pub fn new<S: ToString, I: IntoIterator<Item = (RoleName, AppRoleAssignment)>>(
        installed_app_id: S,
        agent_key: AgentPubKey,
        role_assignments: I,
        manifest: AppManifest,
    ) -> AppResult<Self> {
        let role_assignments: HashMap<_, _> = role_assignments.into_iter().collect();
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
        })
    }

    /// Accessor
    pub fn id(&self) -> &AppId {
        &self.installed_app_id
    }

    /// Accessor
    pub fn provisioned_cells(&self) -> impl Iterator<Item = (&RoleName, &CellId)> {
        self.role_assignments
            .iter()
            .filter_map(|(role_name, role)| role.provisioned_cell().map(|c| (role_name, c)))
    }

    /// Accessor
    pub fn into_provisioned_cells(self) -> impl Iterator<Item = (RoleName, CellId)> {
        self.role_assignments
            .into_iter()
            .filter_map(|(role_name, role)| role.into_provisioned_cell().map(|c| (role_name, c)))
    }

    /// Accessor
    pub fn clone_cells(&self) -> impl Iterator<Item = (&CloneId, &CellId)> {
        self.role_assignments
            .iter()
            .flat_map(|app_role_assignment| app_role_assignment.1.clones.iter())
    }

    /// Accessor
    pub fn disabled_clone_cells(&self) -> impl Iterator<Item = (&CloneId, &CellId)> {
        self.role_assignments
            .iter()
            .flat_map(|app_role_assignment| app_role_assignment.1.disabled_clones.iter())
    }

    /// Accessor
    pub fn clone_cells_for_role_name(
        &self,
        role_name: &RoleName,
    ) -> Option<&HashMap<CloneId, CellId>> {
        match self.role_assignments.get(role_name) {
            None => None,
            Some(role_assignments) => Some(&role_assignments.clones),
        }
    }

    /// Accessor
    pub fn disabled_clone_cells_for_role_name(
        &self,
        role_name: &RoleName,
    ) -> Option<&HashMap<CloneId, CellId>> {
        match self.role_assignments.get(role_name) {
            None => None,
            Some(role_assignment) => Some(&role_assignment.disabled_clones),
        }
    }

    /// Accessor
    pub fn clone_cell_ids(&self) -> impl Iterator<Item = &CellId> {
        self.clone_cells().map(|(_, cell_id)| cell_id)
    }

    /// Accessor
    pub fn disabled_clone_cell_ids(&self) -> impl Iterator<Item = &CellId> {
        self.disabled_clone_cells().map(|(_, cell_id)| cell_id)
    }

    /// Iterator of all cells, both provisioned and cloned
    pub fn all_cells(&self) -> impl Iterator<Item = &CellId> {
        self.provisioned_cells()
            .map(|(_, c)| c)
            .chain(self.clone_cell_ids())
            .chain(self.disabled_clone_cell_ids())
    }

    /// Iterator of all running cells, both provisioned and cloned.
    /// Provisioned cells will always be running if the app is running,
    /// but some cloned cells may be disabled and will not be returned.
    pub fn all_enabled_cells(&self) -> impl Iterator<Item = &CellId> {
        self.provisioned_cells()
            .map(|(_, c)| c)
            .chain(self.clone_cell_ids())
    }

    /// Iterator of all "required" cells, meaning Cells which must be running
    /// for this App to be able to run.
    ///
    /// Currently this is simply all provisioned cells, but this concept may
    /// become more nuanced in the future.
    pub fn required_cells(&self) -> impl Iterator<Item = &CellId> {
        self.provisioned_cells().map(|(_, c)| c)
    }

    /// Accessor for particular role
    pub fn role(&self, role_name: &RoleName) -> AppResult<&AppRoleAssignment> {
        self.role_assignments
            .get(role_name)
            .ok_or_else(|| AppError::RoleNameMissing(role_name.clone()))
    }

    fn role_mut(&mut self, role_name: &RoleName) -> AppResult<&mut AppRoleAssignment> {
        self.role_assignments
            .get_mut(role_name)
            .ok_or_else(|| AppError::RoleNameMissing(role_name.clone()))
    }

    /// Accessor
    pub fn roles(&self) -> &HashMap<RoleName, AppRoleAssignment> {
        &self.role_assignments
    }

    /// Add a clone cell.
    pub fn add_clone(&mut self, role_name: &RoleName, cell_id: &CellId) -> AppResult<CloneId> {
        let app_role_assignment = self.role_mut(role_name)?;

        assert_eq!(
            cell_id.agent_pubkey(),
            app_role_assignment.agent_key(),
            "A clone cell must use the same agent key as the role it is added to"
        );

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
            .insert(clone_id.clone(), cell_id.clone());
        // increment next clone index
        app_role_assignment.next_clone_index += 1;
        Ok(clone_id)
    }

    /// Get a clone cell id from its clone id.
    pub fn get_clone_cell_id(&self, clone_cell_id: &CloneCellId) -> AppResult<CellId> {
        let cell_id = match clone_cell_id {
            CloneCellId::CellId(cell_id) => cell_id,
            CloneCellId::CloneId(clone_id) => self
                .role(&clone_id.as_base_role_name())?
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
            CloneCellId::CellId(id) => {
                self.clone_cells()
                    .find(|(_, cell_id)| *cell_id == id)
                    .ok_or_else(|| AppError::CloneCellNotFound(CloneCellId::CellId(id.clone())))?
                    .0
            }
        };
        Ok(clone_id.clone())
    }

    /// Get the clone id from either clone or cell id.
    pub fn get_disabled_clone_id(&self, clone_cell_id: &CloneCellId) -> AppResult<CloneId> {
        let clone_id = match clone_cell_id {
            CloneCellId::CloneId(id) => id.clone(),
            CloneCellId::CellId(id) => {
                self.role_assignments
                    .iter()
                    .flat_map(|(_, role_assignment)| role_assignment.disabled_clones.clone())
                    .find(|(_, cell_id)| cell_id == id)
                    .ok_or_else(|| AppError::CloneCellNotFound(CloneCellId::CellId(id.clone())))?
                    .0
            }
        };
        Ok(clone_id)
    }

    /// Disable a clone cell.
    ///
    /// Removes the cell from the list of clones, so it is not accessible any
    /// longer. If the cell is already disabled, do nothing and return Ok.
    pub fn disable_clone_cell(&mut self, clone_id: &CloneId) -> AppResult<()> {
        let app_role_assignment = self.role_mut(&clone_id.as_base_role_name())?;
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
        let app_role_assignment = self.role_mut(&clone_id.as_base_role_name())?;
        // remove clone from disabled clones map
        match app_role_assignment.disabled_clones.remove(clone_id) {
            None => app_role_assignment
                .clones
                .get(clone_id)
                .cloned()
                .map(|cell_id| {
                    Ok(InstalledCell {
                        role_name: clone_id.as_app_role_name().to_owned(),
                        cell_id,
                    })
                })
                .unwrap_or_else(|| {
                    Err(AppError::CloneCellNotFound(CloneCellId::CloneId(
                        clone_id.to_owned(),
                    )))
                }),
            Some(cell_id) => {
                // insert clone back into role's clones map
                let insert_result = app_role_assignment
                    .clones
                    .insert(clone_id.to_owned(), cell_id.clone());
                assert!(
                    insert_result.is_none(),
                    "enable: clone cell already enabled"
                );
                Ok(InstalledCell {
                    role_name: clone_id.as_app_role_name().to_owned(),
                    cell_id,
                })
            }
        }
    }

    /// Delete a disabled clone cell.
    pub fn delete_clone_cell(&mut self, clone_id: &CloneId) -> AppResult<()> {
        let app_role_assignment = self.role_mut(&clone_id.as_base_role_name())?;
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
                let role = AppRoleAssignment {
                    base_cell_id: cell_id,
                    is_provisioned: true,
                    clones: HashMap::new(),
                    clone_limit: 256,
                    next_clone_index: 0,
                    disabled_clones: HashMap::new(),
                };
                (role_name, role)
            })
            .collect();

        Ok(Self {
            installed_app_id,
            agent_key: _agent_key,
            role_assignments,
            manifest,
        })
    }

    /// Return the manifest if available
    pub fn manifest(&self) -> &AppManifest {
        &self.manifest
    }

    /// Return the list of role assignments
    pub fn role_assignments(&self) -> &HashMap<RoleName, AppRoleAssignment> {
        &self.role_assignments
    }
}

/// App "roles" correspond to cell entries in the AppManifest.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppRoleAssignment {
    /// The Id of the Cell which will be provisioned for this role.
    /// This also identifies the basis for cloned DNAs, and this is how the
    /// Agent is determined for clones (always the same as the provisioned cell).
    base_cell_id: CellId,
    /// Records whether the base cell has actually been provisioned or not.
    /// If true, then `base_cell_id` refers to an actual existing Cell.
    /// If false, then `base_cell_id` is just recording what that cell will be
    /// called in the future.
    is_provisioned: bool,
    /// The number of allowed clone cells.
    clone_limit: u32,
    /// The index of the next clone cell to be created.
    next_clone_index: u32,
    /// Cells which were cloned at runtime. The length cannot grow beyond
    /// `clone_limit`.
    clones: HashMap<CloneId, CellId>,
    /// Clone cells that have been disabled. These cells cannot be called
    /// any longer and are not returned as part of the app info either.
    /// Disabled clone cells can be deleted through the Admin API.
    disabled_clones: HashMap<CloneId, CellId>,
}

/// Ways of specifying a clone cell.
#[derive(Clone, Debug, derive_more::Display, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum CloneCellId {
    /// Clone id consisting of role name and clone index.
    CloneId(CloneId),
    /// Cell id consisting of DNA hash and agent pub key.
    CellId(CellId),
}

impl AppRoleAssignment {
    /// Constructor. List of clones always starts empty.
    pub fn new(base_cell_id: CellId, is_provisioned: bool, clone_limit: u32) -> Self {
        Self {
            base_cell_id,
            is_provisioned,
            clone_limit,
            clones: HashMap::new(),
            next_clone_index: 0,
            disabled_clones: HashMap::new(),
        }
    }

    /// Accessor
    pub fn cell_id(&self) -> &CellId {
        &self.base_cell_id
    }

    /// Accessor
    pub fn dna_hash(&self) -> &DnaHash {
        self.base_cell_id.dna_hash()
    }

    /// Accessor
    pub fn agent_key(&self) -> &AgentPubKey {
        self.base_cell_id.agent_pubkey()
    }

    /// Accessor
    pub fn provisioned_cell(&self) -> Option<&CellId> {
        if self.is_provisioned {
            Some(&self.base_cell_id)
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
    use super::*;
    use ::fixt::*;
    use arbitrary::Arbitrary;
    use std::collections::HashSet;

    use holochain_zome_types::fixt::*;

    #[test]
    fn illegal_role_name_is_rejected() {
        let mut u = unstructured_noise();
        let result = InstalledAppCommon::new(
            "test_app",
            fixt!(AgentPubKey),
            vec![(
                CLONE_ID_DELIMITER.into(),
                AppRoleAssignment::new(fixt!(CellId), false, 0),
            )],
            AppManifest::arbitrary(&mut u).unwrap(),
        );
        assert!(result.is_err())
    }

    #[test]
    fn clone_management() {
        let base_cell_id = fixt!(CellId);
        let agent = base_cell_id.agent_pubkey().clone();
        let new_clone = || CellId::new(fixt!(DnaHash), agent.clone());
        let clone_limit = 3;
        let role1 = AppRoleAssignment::new(base_cell_id, false, clone_limit);
        let agent = fixt!(AgentPubKey);
        let role_name: RoleName = "role_name".into();
        let manifest = AppManifest::arbitrary(&mut unstructured_noise()).unwrap();
        let mut app: RunningApp = InstalledAppCommon::new(
            "app",
            agent.clone(),
            vec![(role_name.clone(), role1)],
            manifest,
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
            app.clone_cell_ids().collect::<HashSet<_>>(),
            maplit::hashset! { &clones[0], &clones[1], &clones[2] }
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
            .find(|(clone_id, _)| **clone_id == clone_id_0)
            .is_some());
        assert_eq!(
            app.clone_cell_ids().collect::<HashSet<_>>(),
            maplit::hashset! { &clones[0], &clones[1], &clones[2] }
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
