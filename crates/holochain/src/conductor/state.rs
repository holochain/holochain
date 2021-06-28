//! Structs which allow the Conductor's state to be persisted across
//! startups and shutdowns

use holochain_conductor_api::signal_subscription::SignalSubscription;
use holochain_conductor_api::{config::InterfaceDriver, InstalledAppInfo};
use holochain_types::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

use super::error::{ConductorError, ConductorResult};

/// Mutable conductor state, stored in a DB and writable only via Admin interface.
///
/// References between structs (cell configs pointing to
/// the agent and DNA to be instantiated) are implemented
/// via string IDs.
#[derive(Clone, Deserialize, Serialize, Default, Debug, SerializedBytes)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ConductorState {
    /// Apps that have been installed, regardless of status
    #[serde(default)]
    installed_apps: InstalledAppMap,
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

impl ConductorState {
    /// Immutable access to the inner collection of all apps
    pub fn installed_apps(&self) -> &InstalledAppMap {
        &self.installed_apps
    }

    /// Mutable access to the inner collection of all apps
    // #[cfg(test)]
    #[deprecated = "Bare mutable access isn't the best idea"]
    pub fn installed_apps_mut(&mut self) -> &mut InstalledAppMap {
        &mut self.installed_apps
    }

    /// Iterate over only the "enabled" apps
    pub fn enabled_apps(&self) -> impl Iterator<Item = (&InstalledAppId, &InstalledApp)> + '_ {
        self.installed_apps
            .iter()
            .filter(|(_, app)| app.status().is_enabled())
    }

    /// Iterate over only the "disabled" apps
    pub fn disabled_apps(&self) -> impl Iterator<Item = (&InstalledAppId, &InstalledApp)> + '_ {
        self.installed_apps
            .iter()
            .filter(|(_, app)| !app.status().is_enabled())
    }

    /// Iterate over only the "running" apps
    pub fn running_apps(&self) -> impl Iterator<Item = (&InstalledAppId, RunningApp)> + '_ {
        self.installed_apps.iter().filter_map(|(id, app)| {
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
        self.installed_apps
            .iter()
            .filter_map(|(id, app)| if app.status.is_paused() {
                StoppedApp::from_app(app).map(|stopped| (id, stopped)) 
            } else {
                None
            })
    }

    /// Iterate over only the "stopped" apps (paused OR disabled)
    pub fn stopped_apps(&self) -> impl Iterator<Item = (&InstalledAppId, StoppedApp)> + '_ {
        self.installed_apps
            .iter()
            .filter_map(|(id, app)| StoppedApp::from_app(app).map(|stopped| (id, stopped)))
    }

    /// Getter for a single app. Returns error if app missing.
    pub fn get_app(&self, id: &InstalledAppId) -> ConductorResult<&InstalledApp> {
        self.installed_apps
            .get(id)
            .ok_or_else(|| ConductorError::AppNotInstalled(id.clone()))
    }

    /// Getter for a single app. Returns error if app missing.
    pub fn remove_app(&mut self, id: &InstalledAppId) -> ConductorResult<InstalledApp> {
        self.installed_apps
            .remove(id)
            .ok_or_else(|| ConductorError::AppNotInstalled(id.clone()))
    }

    /// Add an app in the Deactivated state. Returns an error if an app is already
    /// present at the given ID.
    pub fn add_app(&mut self, app: InstalledAppCommon) -> ConductorResult<StoppedApp> {
        if self.installed_apps.contains_key(app.id()) {
            return Err(ConductorError::AppAlreadyInstalled(app.id().clone()));
        }
        let stopped_app = StoppedApp::new_fresh(app);
        self.installed_apps.insert(stopped_app.clone().into());
        Ok(stopped_app)
    }

    /// Update the status of an installed app in-place.
    /// Return a reference to the (possibly updated) app.
    /// Additionally, if an update occurred, return the previous state. If no update occurred, return None.
    pub fn transition_app_status(
        &mut self,
        id: &InstalledAppId,
        transition: AppStatusTransition,
    ) -> ConductorResult<(&InstalledApp, AppStatusRunningDelta)> {
        let app = self
            .installed_apps
            .get_mut(id)
            .ok_or_else(|| ConductorError::AppNotInstalled(id.clone()))?;
        let delta = app.status.transition(transition);
        Ok((app, delta))
    }

    /// Retrieve info about an installed App by its InstalledAppId
    #[allow(clippy::ptr_arg)]
    pub fn get_app_info(&self, installed_app_id: &InstalledAppId) -> Option<InstalledAppInfo> {
        self.installed_apps
            .get(installed_app_id)
            .map(InstalledAppInfo::from_installed_app)
    }

    /// Returns the interface configuration with the given ID if present
    pub fn interface_by_id(&self, id: &AppInterfaceId) -> Option<AppInterfaceConfig> {
        self.app_interfaces.get(id).cloned()
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

    /// The driver for the interface, e.g. Websocket
    pub driver: InterfaceDriver,
}

impl AppInterfaceConfig {
    /// Create config for a websocket interface
    pub fn websocket(port: u16) -> Self {
        Self {
            signal_subscriptions: HashMap::new(),
            driver: InterfaceDriver::Websocket { port },
        }
    }
}

// TODO: Tons of consistency check tests were ripped out in the great legacy code cleanup
// We need to add these back in when we've landed the new Dna format
// See https://github.com/holochain/holochain/blob/7750a0291e549be006529e4153b3b6cf0d686462/crates/holochain/src/conductor/state/tests.rs#L1
// for all old tests
