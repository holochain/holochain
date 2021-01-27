//! Structs which allow the Conductor's state to be persisted across
//! startups and shutdowns

use holochain_conductor_api::config::InterfaceDriver;
use holochain_conductor_api::signal_subscription::SignalSubscription;
use holochain_types::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Mutable conductor state, stored in a DB and writeable only via Admin interface.
///
/// References between structs (cell configs pointing to
/// the agent and DNA to be instantiated) are implemented
/// via string IDs.
#[derive(Clone, Deserialize, Serialize, Default, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ConductorState {
    /// Apps that are ready to be activated
    #[serde(default)]
    pub inactive_apps: HashMap<InstalledAppId, Vec<InstalledCell>>,
    /// Apps that are active and will be loaded
    #[serde(default)]
    pub active_apps: HashMap<InstalledAppId, Vec<InstalledCell>>,
    /// List of interfaces any UI can use to access zome functions.
    #[serde(default)]
    pub app_interfaces: HashMap<AppInterfaceId, AppInterfaceConfig>,
}

/// A unique identifier used to refer to an App Interface internally.
#[derive(
    Clone,
    Deserialize,
    Serialize,
    Default,
    Debug,
    Hash,
    PartialEq,
    Eq,
    derive_more::From,
    derive_more::Display,
)]
pub struct AppInterfaceId(String);

impl From<&str> for AppInterfaceId {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl ConductorState {
    /// Retrieve info about an installed App by its InstalledAppId
    #[allow(clippy::ptr_arg)]
    pub fn get_app_info(&self, installed_app_id: &InstalledAppId) -> Option<InstalledApp> {
        self.active_apps
            .get(installed_app_id)
            .or_else(|| self.inactive_apps.get(installed_app_id))
            .map(|cell_data| InstalledApp {
                installed_app_id: installed_app_id.clone(),
                cell_data: cell_data.clone(),
            })
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
