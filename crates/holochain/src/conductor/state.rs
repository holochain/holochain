//! Structs which allow the Conductor's state to be persisted across
//! startups and shutdowns

use holochain_conductor_api::config::InterfaceDriver;
use holochain_conductor_api::signal_subscription::SignalSubscription;
use holochain_p2p::NetworkCompatParams;
use holochain_types::prelude::*;
use holochain_types::websocket::AllowedOrigins;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use super::error::{ConductorError, ConductorResult};

/// Unique conductor tag / identifier.
#[derive(Clone, Deserialize, Serialize, Debug, SerializedBytes)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(transparent)]
pub struct ConductorStateTag(pub Arc<str>);

impl Default for ConductorStateTag {
    fn default() -> Self {
        Self(nanoid::nanoid!().into())
    }
}

/// Mutable conductor state, stored in a DB and writable only via Admin interface.
///
/// References between structs (cell configs pointing to
/// the agent and DNA to be instantiated) are implemented
/// via string IDs.
#[serde_with::serde_as]
#[derive(Clone, Deserialize, Serialize, Default, Debug, SerializedBytes)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ConductorState {
    /// Unique conductor tag / identifier.
    #[serde(default)]
    tag: ConductorStateTag,
    /// Apps (and services) that have been installed, regardless of status.
    #[serde(default)]
    installed_apps: InstalledAppMap,

    /// List of interfaces any UI can use to access zome functions.
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(default)]
    pub(crate) app_interfaces: HashMap<AppInterfaceId, AppInterfaceConfig>,
}

/// A unique identifier used to refer to an App Interface internally.
#[derive(Clone, Deserialize, Serialize, Debug, Hash, PartialEq, Eq)]
pub struct AppInterfaceId {
    /// The port used to create this interface
    port: u16,
    /// If the port is 0 then it will be assigned by the OS
    /// so we need a unique identifier for that case.
    id: Option<String>,
}

impl Default for AppInterfaceId {
    fn default() -> Self {
        Self::new(0)
    }
}

impl AppInterfaceId {
    /// Create an id from the port
    pub fn new(port: u16) -> Self {
        let id = if port == 0 {
            Some(nanoid::nanoid!())
        } else {
            None
        };
        Self { port, id }
    }
    /// Get the port intended for this interface
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl ConductorState {
    /// A unique identifier for this conductor
    pub fn tag(&self) -> &ConductorStateTag {
        &self.tag
    }

    /// Set the tag for this conductor
    #[cfg(test)]
    pub fn set_tag(&mut self, tag: ConductorStateTag) {
        self.tag = tag;
    }

    /// Immutable access to the inner collection of all apps and services
    pub fn installed_apps(&self) -> &InstalledAppMap {
        &self.installed_apps
    }

    /// Mutable access to the inner collection of all apps
    #[deprecated = "Bare mutable access isn't the best idea"]
    pub fn installed_apps_mut(&mut self) -> &mut InstalledAppMap {
        &mut self.installed_apps
    }

    /// Iterate over only the "enabled" apps
    pub fn enabled_apps(&self) -> impl Iterator<Item = (&InstalledAppId, &InstalledApp)> + '_ {
        self.installed_apps
            .iter()
            .filter(|(_, app)| app.status == AppStatus::Enabled)
    }

    /// Iterate over only the "disabled" apps
    pub fn disabled_apps(&self) -> impl Iterator<Item = (&InstalledAppId, &InstalledApp)> + '_ {
        self.installed_apps
            .iter()
            .filter(|(_, app)| matches!(app.status, AppStatus::Disabled(_)))
    }

    /// Getter for a single app. Returns error if app missing.
    pub fn get_app(&self, id: &InstalledAppId) -> ConductorResult<&InstalledApp> {
        self.installed_apps
            .get(id)
            .ok_or_else(|| ConductorError::AppNotInstalled(id.clone()))
    }

    /// Getter for a mutable reference to a single app. Returns error if app missing.
    pub fn get_app_mut(&mut self, id: &InstalledAppId) -> ConductorResult<&mut InstalledApp> {
        self.installed_apps
            .get_mut(id)
            .ok_or_else(|| ConductorError::AppNotInstalled(id.clone()))
    }

    /// Getter for a single app. Returns error if app missing.
    pub fn remove_app(&mut self, id: &InstalledAppId) -> ConductorResult<InstalledApp> {
        self.installed_apps
            .swap_remove(id)
            .ok_or_else(|| ConductorError::AppNotInstalled(id.clone()))
    }

    /// Add an app in the Disabled state. Returns an error if an app is already
    /// present at the given ID.
    pub fn add_app(&mut self, app: InstalledAppCommon) -> ConductorResult<InstalledApp> {
        if self.installed_apps.contains_key(app.id()) {
            return Err(ConductorError::AppAlreadyInstalled(app.id().clone()));
        }
        let app = InstalledApp::new(app, AppStatus::Disabled(DisabledAppReason::NeverStarted));
        self.installed_apps.insert(app.id().clone(), app.clone());
        Ok(app)
    }

    /// Add an app in the AwaitingMemproofs state. Returns an error if an app is already
    /// present at the given ID.
    pub fn add_app_awaiting_memproofs(
        &mut self,
        app: InstalledAppCommon,
    ) -> ConductorResult<InstalledApp> {
        if self.installed_apps.contains_key(app.id()) {
            return Err(ConductorError::AppAlreadyInstalled(app.id().clone()));
        }
        let app = InstalledApp::new(app, AppStatus::AwaitingMemproofs);
        self.installed_apps.insert(app.id().clone(), app.clone());
        Ok(app)
    }

    /// Returns the interface configuration with the given ID if present
    pub fn interface_by_id(&self, id: &AppInterfaceId) -> Option<AppInterfaceConfig> {
        self.app_interfaces.get(id).cloned()
    }

    /// Find the app which contains the given cell by its [CellId].
    pub fn find_app_containing_cell(&self, cell_id: &CellId) -> Option<&InstalledApp> {
        self.installed_apps
            .values()
            .find(|app| app.all_cells().any(|id| id == *cell_id))
    }

    /// Get network compability params
    /// (but this can't actually be on the Conductor since it must be retrieved before
    /// conductor initialization)
    pub fn get_network_compat(&self) -> NetworkCompatParams {
        tracing::warn!("Using default NetworkCompatParams");
        Default::default()
    }

    /// Find all installed apps that have a role which depends on a cell in this app
    /// via `AppRoleAssignment::Dependency`.
    ///
    /// The `protected_only` field is a filter. If false, all dependent apps are returned.
    /// If true, only dependent apps with at least one protected dependency are returned.
    pub fn get_dependent_apps(
        &self,
        id: &InstalledAppId,
        protected_only: bool,
    ) -> ConductorResult<Vec<InstalledAppId>> {
        let app = self.get_app(id)?;
        let cell_ids: HashSet<_> = app.all_cells().collect();
        Ok(self
            .installed_apps
            .iter()
            .filter(|(_, app)| {
                app.role_assignments.values().any(|r| match r {
                    AppRoleAssignment::Primary(_) => false,
                    AppRoleAssignment::Dependency(d) => {
                        cell_ids.contains(&d.cell_id) && (!protected_only || d.protected)
                    }
                })
            })
            .map(|(id, _)| id.clone())
            .collect())
    }
}

/// Here, interfaces are user facing and make available zome functions to
/// GUIs, browser based web UIs, local native UIs, other local applications and scripts.
/// We currently have:
/// * websockets
///
/// We will also soon develop
/// * Unix domain sockets
///
/// The cells (referenced by ID) that are to be made available via that interface should be listed.
#[derive(Clone, Deserialize, Serialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct AppInterfaceConfig {
    /// The signal subscription settings for each App
    pub signal_subscriptions: HashMap<InstalledAppId, SignalSubscription>,

    /// The application that this interface is for. If `Some`, then this interface will only allow
    /// connections which use a token that has been issued for the same app id. Otherwise, any app
    /// is allowed to connect.
    pub installed_app_id: Option<InstalledAppId>,

    /// The driver for the interface, e.g. Websocket
    pub driver: InterfaceDriver,
}

impl AppInterfaceConfig {
    /// Create config for a websocket interface
    pub fn websocket(
        port: u16,
        allowed_origins: AllowedOrigins,
        installed_app_id: Option<InstalledAppId>,
    ) -> Self {
        Self {
            signal_subscriptions: HashMap::new(),
            installed_app_id,
            driver: InterfaceDriver::Websocket {
                port,
                allowed_origins,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ConductorState;
    use ::fixt::fixt;
    use hdk::prelude::CellId;
    use holo_hash::fixt::{AgentPubKeyFixturator, DnaHashFixturator};
    use holochain_timestamp::Timestamp;
    use holochain_types::app::{
        AppManifestV0Builder, AppRoleAssignment, AppRolePrimary, AppStatus, DisabledAppReason,
        InstalledApp, InstalledAppCommon,
    };

    #[test]
    fn app_status() {
        let mut state = ConductorState::default();
        let agent = fixt!(AgentPubKey);
        let dna_hash = fixt!(DnaHash);
        let cell_id = CellId::new(dna_hash.clone(), agent.clone());
        assert_eq!(state.enabled_apps().count(), 0);
        assert_eq!(state.disabled_apps().count(), 0);
        assert_eq!(state.installed_apps().len(), 0);
        assert!(state.find_app_containing_cell(&cell_id).is_none());

        // Add an app
        let app_manifest = AppManifestV0Builder::default()
            .name("name".to_string())
            .description(None)
            // cell lookup is purely based on the role assignments of the InstalledAppCommon
            .roles(vec![])
            .build()
            .unwrap();
        let app_id = "app_id";
        let app = InstalledAppCommon::new(
            app_id,
            agent.clone(),
            [(
                "role_1".to_string(),
                AppRoleAssignment::Primary(AppRolePrimary::new(dna_hash, true, 0)),
            )],
            app_manifest.into(),
            Timestamp::now(),
        )
        .unwrap();
        state.add_app(app.clone()).unwrap();
        assert_eq!(state.enabled_apps().count(), 0);
        assert_eq!(state.disabled_apps().count(), 1);
        assert_eq!(state.installed_apps().len(), 1);
        let installed_app = InstalledApp::new(
            app.clone(),
            AppStatus::Disabled(DisabledAppReason::NeverStarted),
        );
        assert_eq!(
            state.installed_apps().first().unwrap(),
            (&app_id.to_string(), &installed_app)
        );
        assert_eq!(
            state.find_app_containing_cell(&cell_id).unwrap(),
            &installed_app
        );

        // Set app state to enabled
        state.get_app_mut(&app_id.to_string()).unwrap().status = AppStatus::Enabled;
        assert_eq!(state.enabled_apps().count(), 1);
        assert_eq!(state.disabled_apps().count(), 0);
        assert_eq!(state.installed_apps().len(), 1);
        let installed_app = InstalledApp::new(app.clone(), AppStatus::Enabled);
        assert_eq!(
            state.installed_apps().first().unwrap(),
            (&app_id.to_string(), &installed_app)
        );
        assert_eq!(
            state.find_app_containing_cell(&cell_id).unwrap(),
            &installed_app
        );
    }
}
