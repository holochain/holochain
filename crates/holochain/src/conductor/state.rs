use crate::conductor::interface::InterfaceDriver;

use holochain_types::{
    app::{AppId, InstalledApp, InstalledCell},
    cell::CellId,
    dna::error::DnaError,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Mutable conductor state, stored in a DB and writeable only via Admin interface.
///
/// References between structs (cell configs pointing to
/// the agent and DNA to be instantiated) are implemented
/// via string IDs.
#[derive(Deserialize, Serialize, Clone, PartialEq, Default, Debug)]
pub struct ConductorState {
    /// Apps that are ready to be activated
    #[serde(default)]
    pub inactive_apps: HashMap<AppId, Vec<InstalledCell>>,
    /// Apps that are active and will be loaded
    #[serde(default)]
    pub active_apps: HashMap<AppId, Vec<InstalledCell>>,
    /// List of interfaces any UI can use to access zome functions.
    #[serde(default)]
    pub interfaces: HashMap<InterfaceId, InterfaceConfig>,
}

pub type InterfaceId = String;

impl ConductorState {
    pub fn check_consistency(&self) -> Result<(), DnaError> {
        // FIXME: A huge amount of legacy code for checking the consistency of Dna was ripped out here
        // let's make sure we get that back in once we land the Dna and Zome structure.
        Ok(())
    }

    pub fn get_app_info(&self, app_id: &AppId) -> Option<InstalledApp> {
        self.active_apps
            .get(app_id)
            .or_else(|| self.inactive_apps.get(app_id))
            .map(|cell_data| InstalledApp {
                app_id: app_id.clone(),
                cell_data: cell_data.clone(),
            })
    }

    /// Returns the interface configuration with the given ID if present
    pub fn interface_by_id(&self, id: &str) -> Option<InterfaceConfig> {
        self.interfaces.get(id).cloned()
    }
}

/// Here, interfaces are user facing and make available zome functions to
/// GUIs, browser based web UIs, local native UIs, other local applications and scripts.
/// We currently have:
/// * websockets
/// * HTTP
///
/// We will also soon develop
/// * Unix domain sockets
///
/// The cells (referenced by ID) that are to be made available via that interface should be listed.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct InterfaceConfig {
    pub cells: Vec<CellId>,
    pub driver: InterfaceDriver,
}

// TODO: Tons of consistency check tests were ripped out in the great legacy code cleanup
// We need to add these back in when we've landed the new Dna format
// See https://github.com/Holo-Host/holochain-2020/blob/7750a0291e549be006529e4153b3b6cf0d686462/crates/holochain/src/conductor/state/tests.rs#L1
// for all old tests
