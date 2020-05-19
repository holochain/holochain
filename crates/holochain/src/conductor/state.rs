use crate::conductor::interface::InterfaceDriver;
use holochain_types::{cell::CellId, dna::error::DnaError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Mutable conductor state, stored in a DB and writeable only via Admin interface.
///
/// References between structs (cell configs pointing to
/// the agent and DNA to be instantiated) are implemented
/// via string IDs.
#[derive(Deserialize, Serialize, Clone, PartialEq, Default, Debug)]
pub struct ConductorState {
    // TODO: B-01610: Maybe we shouldn't store proofs here
    /// List of cell IDs, includes references to an agent and a DNA. Optional.
    #[serde(default)]
    pub cell_ids: Vec<CellId>,

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

    /// Returns the interface configuration with the given ID if present
    pub fn interface_by_id(&self, id: &str) -> Option<InterfaceConfig> {
        self.interfaces.get(id).cloned()
    }

    /// Returns all defined cell IDs
    pub fn cell_ids(&self) -> &Vec<CellId> {
        &self.cell_ids
    }

    /// Removes the cell given by id and all mentions of it in other elements so
    /// that the config is guaranteed to be valid afterwards if it was before.
    pub fn save_remove_cell(mut self, id: &CellId) -> Self {
        self.cell_ids.retain(|cell| cell != id);

        self.interfaces = self
            .interfaces
            .into_iter()
            .map(|(interface_id, mut interface)| {
                interface.cells.retain(|cell_id| cell_id != id);
                (interface_id, interface)
            })
            .collect();

        self
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
