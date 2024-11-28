//! Structs which allow the Conductor's state to be persisted across
//! startups and shutdowns

use holochain_conductor_api::config::InterfaceDriver;
use holochain_conductor_api::signal_subscription::SignalSubscription;
use holochain_conductor_services::DeepkeyInstallation;
use holochain_conductor_services::DPKI_APP_ID;
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

/// Info required to re-initialize conductor services upon restart
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize, Default, Debug, SerializedBytes)]
pub struct ConductorServicesState {
    /// Data needed to initialize the DPKI service, if installed
    pub dpki: Option<DeepkeyInstallation>,
}

/// Mutable conductor state, stored in a DB and writable only via Admin interface.
///
/// References between structs (cell configs pointing to
/// the agent and DNA to be instantiated) are implemented
/// via string IDs.
#[derive(Clone, Deserialize, Serialize, Default, Debug, SerializedBytes)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ConductorState {
    /// Unique conductor tag / identifier.
    #[serde(default)]
    tag: ConductorStateTag,
    /// Apps (and services) that have been installed, regardless of status.
    #[serde(default)]
    installed_apps_and_services: InstalledAppMap,

    /// Number of agent keys that have ever been derived from the device seed.
    /// Only increases, never decreases. Used for deriving reconstructible
    /// agent keys from the lair "device seed".
    #[serde(default)]
    pub derived_agent_key_count: u32,

    /// Conductor services that have been installed, to enable initialization
    /// upon restart
    #[serde(default)]
    pub(crate) conductor_services: ConductorServicesState,

    /// List of interfaces any UI can use to access zome functions.
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

/// Does the given InstalledAppId refer to an app, or a service?
pub fn is_app(id: &InstalledAppId) -> bool {
    !is_service(id)
}

/// Does the given InstalledAppId refer to a service, or an app?
pub fn is_service(id: &InstalledAppId) -> bool {
    id.as_str() == DPKI_APP_ID
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
    pub fn installed_apps_and_services(&self) -> &InstalledAppMap {
        &self.installed_apps_and_services
    }

    /// Mutable access to the inner collection of all apps
    // #[cfg(test)]
    #[deprecated = "Bare mutable access isn't the best idea"]
    pub fn installed_apps_and_services_mut(&mut self) -> &mut InstalledAppMap {
        &mut self.installed_apps_and_services
    }

    /// Iterate over only the "enabled" apps and services
    pub fn enabled_apps_and_services(
        &self,
    ) -> impl Iterator<Item = (&InstalledAppId, &InstalledApp)> + '_ {
        self.installed_apps_and_services
            .iter()
            .filter(|(_, app)| app.status().is_enabled())
    }

    /// Iterate over only the "enabled" apps
    pub fn enabled_apps(&self) -> impl Iterator<Item = (&InstalledAppId, &InstalledApp)> + '_ {
        self.installed_apps_and_services
            .iter()
            .filter(|(_, app)| app.status().is_enabled())
            .filter(|(id, _)| is_app(id))
    }

    /// Iterate over only the "disabled" apps
    pub fn disabled_apps(&self) -> impl Iterator<Item = (&InstalledAppId, &InstalledApp)> + '_ {
        self.installed_apps_and_services
            .iter()
            .filter(|(id, _)| is_app(id))
            .filter(|(_, app)| !app.status().is_enabled())
    }

    /// Iterate over only the "running" apps
    pub fn running_apps(&self) -> impl Iterator<Item = (&InstalledAppId, RunningApp)> + '_ {
        self.installed_apps_and_services
            .iter()
            .filter(|(id, _)| is_app(id))
            .filter_map(|(id, app)| {
                if *app.status() == AppStatus::Running {
                    let running = RunningApp::from(app.as_ref().clone());
                    Some((id, running))
                } else {
                    None
                }
            })
    }

    /// Iterate over only the paused apps
    pub fn paused_apps(&self) -> impl Iterator<Item = (&InstalledAppId, StoppedApp)> + '_ {
        self.installed_apps_and_services
            .iter()
            .filter(|(id, _)| is_app(id))
            .filter_map(|(id, app)| {
                if app.status.is_paused() {
                    StoppedApp::from_app(app).map(|stopped| (id, stopped))
                } else {
                    None
                }
            })
    }

    /// Iterate over only the "stopped" apps (paused OR disabled)
    pub fn stopped_apps(&self) -> impl Iterator<Item = (&InstalledAppId, StoppedApp)> + '_ {
        self.installed_apps_and_services
            .iter()
            .filter(|(id, _)| is_app(id))
            .filter_map(|(id, app)| StoppedApp::from_app(app).map(|stopped| (id, stopped)))
    }

    /// Getter for a single app. Returns error if app missing.
    pub fn get_app(&self, id: &InstalledAppId) -> ConductorResult<&InstalledApp> {
        self.installed_apps_and_services
            .get(id)
            .ok_or_else(|| ConductorError::AppNotInstalled(id.clone()))
    }

    /// Getter for a mutable reference to a single app. Returns error if app missing.
    pub fn get_app_mut(&mut self, id: &InstalledAppId) -> ConductorResult<&mut InstalledApp> {
        self.installed_apps_and_services
            .get_mut(id)
            .ok_or_else(|| ConductorError::AppNotInstalled(id.clone()))
    }

    /// Getter for a single app. Returns error if app missing.
    pub fn remove_app(&mut self, id: &InstalledAppId) -> ConductorResult<InstalledApp> {
        self.installed_apps_and_services
            .remove(id)
            .ok_or_else(|| ConductorError::AppNotInstalled(id.clone()))
    }

    /// Add an app in the Disabled state. Returns an error if an app is already
    /// present at the given ID.
    pub fn add_app(&mut self, app: InstalledAppCommon) -> ConductorResult<StoppedApp> {
        if self.installed_apps_and_services.contains_key(app.id()) {
            return Err(ConductorError::AppAlreadyInstalled(app.id().clone()));
        }
        let stopped_app = StoppedApp::new_fresh(app);
        self.installed_apps_and_services
            .insert(stopped_app.clone().into());
        Ok(stopped_app)
    }

    /// Add an app in the AwaitingMemproofs state. Returns an error if an app is already
    /// present at the given ID.
    pub fn add_app_awaiting_memproofs(
        &mut self,
        app: InstalledAppCommon,
    ) -> ConductorResult<InstalledApp> {
        if self.installed_apps_and_services.contains_key(app.id()) {
            return Err(ConductorError::AppAlreadyInstalled(app.id().clone()));
        }
        let app = InstalledApp::new(app, AppStatus::AwaitingMemproofs);
        self.installed_apps_and_services.insert(app.clone());
        Ok(app)
    }

    /// Update the status of an installed app in-place.
    /// Return a reference to the (possibly updated) app.
    /// Additionally, if an update occurred, return the previous state. If no update occurred, return None.
    pub fn transition_app_status(
        &mut self,
        id: &InstalledAppId,
        transition: AppStatusTransition,
    ) -> ConductorResult<(&InstalledApp, AppStatusFx)> {
        match transition {
            AppStatusTransition::Disable(_) | AppStatusTransition::Pause(_) => {
                let dependents: Vec<_> = self
                    .get_dependent_apps(id, true)?
                    .into_iter()
                    .filter(|id| {
                        self.installed_apps_and_services
                            .get(id)
                            .map(|app| app.status().is_running())
                            .unwrap_or(false)
                    })
                    .collect();
                if !dependents.is_empty() {
                    tracing::warn!(
                        "Disabling/pausing app '{}' which has running protected dependent apps: {:?}",
                        id,
                        dependents
                    );
                }
            }
            AppStatusTransition::Enable | AppStatusTransition::Start => {
                let dependencies: Vec<_> = self
                    .get_app(id)?
                    .roles()
                    .values()
                    .flat_map(|r| match r {
                        AppRoleAssignment::Primary(_) => vec![],
                        AppRoleAssignment::Dependency(AppRoleDependency { cell_id, protected }) => {
                            if *protected {
                                self.installed_apps_and_services
                                    .iter()
                                    .filter_map(|(id, app)| {
                                        (!app.status().is_running()
                                            && app.all_cells().any(|id| id == *cell_id))
                                        .then_some(id)
                                    })
                                    .collect()
                            } else {
                                vec![]
                            }
                        }
                    })
                    .collect();
                if !dependencies.is_empty() {
                    return Err(ConductorError::AppStatusError(format!(
                        "Enabling/starting App '{}' which has protected dependencies that are not running: {:?}",
                        id, dependencies
                    )));
                }
            }
        }
        let app = self
            .installed_apps_and_services
            .get_mut(id)
            .ok_or_else(|| ConductorError::AppNotInstalled(id.clone()))?;
        let delta = app.status.transition(transition);
        Ok((app, delta))
    }

    /// Returns the interface configuration with the given ID if present
    pub fn interface_by_id(&self, id: &AppInterfaceId) -> Option<AppInterfaceConfig> {
        self.app_interfaces.get(id).cloned()
    }

    /// Find the app which contains the given cell by its [CellId].
    pub fn find_app_containing_cell(&self, cell_id: &CellId) -> Option<&InstalledApp> {
        self.installed_apps_and_services
            .values()
            .find(|app| app.all_cells().any(|id| id == *cell_id))
    }

    /// Get network compability params
    /// (but this can't actually be on the Conductor since it must be retrieved before
    /// conductor initialization)
    pub fn get_network_compat(&self) -> NetworkCompatParams {
        NetworkCompatParams {
            dpki_uuid: {
                tracing::warn!("Using default NetworkCompatParams");
                None
            },
        }
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
            .installed_apps_and_services
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

    /// Find all installed apps which have a cell that this app depends on
    /// via `AppRoleAssignment::Dependency`.
    ///
    /// The `protected_only` field is a filter. If false, all dependency apps are returned.
    /// If true, only protected dependencies are returned.
    pub fn get_depdency_apps(
        &self,
        id: &InstalledAppId,
        protected_only: bool,
    ) -> ConductorResult<Vec<InstalledAppId>> {
        let dependencies: Vec<_> = self
            .get_app(id)?
            .roles()
            .values()
            .flat_map(|r| match r {
                AppRoleAssignment::Primary(_) => vec![],
                AppRoleAssignment::Dependency(AppRoleDependency { cell_id, protected }) => {
                    if !protected_only || *protected {
                        self.installed_apps_and_services
                            .iter()
                            .filter_map(|(id, app)| {
                                (app.all_cells().any(|id| id == *cell_id)
                                    && !app.status().is_running())
                                .then_some(id)
                            })
                            .collect()
                    } else {
                        vec![]
                    }
                }
            })
            .cloned()
            .collect();
        Ok(dependencies)
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

// TODO: Tons of consistency check tests were ripped out in the great legacy code cleanup
// We should add these back in when we've landed the new Dna format
// See https://github.com/holochain/holochain/blob/7750a0291e549be006529e4153b3b6cf0d686462/crates/holochain/src/conductor/state/tests.rs#L1
// for all old tests
