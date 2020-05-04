use crate::conductor::interface::InterfaceDriver;

use holochain_types::{
    dna::{error::DnaError, DnaFile},
    prelude::*,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf, sync::Arc};

/// Mutable conductor state, stored in a DB and writeable only via Admin interface.
///
/// References between structs (cell configs pointing to
/// the agent and DNA to be instantiated) are implemented
/// via string IDs.
#[derive(Deserialize, Serialize, Clone, PartialEq, Default, Debug)]
pub struct ConductorState {
    /// List of Agents, this mainly means identities and their keys. Required.
    pub agents: Vec<AgentConfig>,
    /// List of DNAs, for each a path to the DNA file. Optional.
    #[serde(default)]
    pub dnas: Vec<DnaConfig>,
    /// List of cells, includes references to an agent and a DNA. Optional.
    #[serde(default)]
    pub cells: Vec<CellConfig>,
    /// List of interfaces any UI can use to access zome functions. Optional.
    #[serde(default)]
    pub interfaces: Vec<InterfaceConfig>,
}

/// Check for duplicate items in a list of strings
fn _detect_dupes<'a, I: Iterator<Item = &'a String>>(
    name: &'static str,
    items: I,
) -> Result<(), String> {
    let mut set = HashSet::<&str>::new();
    let mut dupes = Vec::<String>::new();
    for item in items {
        if !set.insert(item) {
            dupes.push(item.to_string())
        }
    }
    if !dupes.is_empty() {
        Err(format!(
            "Duplicate {} IDs detected: {}",
            name,
            dupes.join(", ")
        ))
    } else {
        Ok(())
    }
}


impl ConductorState {
    pub fn check_consistency(&self) -> Result<(), DnaError> {
        // TODO: A huge amount of legacy code for checking the consistency of Dna was ripped out here
        // let's make sure we get that back in once we land the Dna and Zome structure.
        Ok(())
    }

    /// Returns the agent configuration with the given ID if present
    pub fn agent_by_id(&self, id: &str) -> Option<AgentConfig> {
        self.agents.iter().find(|ac| ac.id == *id).cloned()
    }

    /// Returns the agent configuration with the given ID if present
    pub fn update_agent_address_by_id(&mut self, id: &str, agent_pubkey: &AgentPubKey) {
        self.agents.iter_mut().for_each(|ac| {
            if ac.id == *id {
                ac.hash = agent_pubkey.clone()
            }
        })
    }

    /// Returns the DNA configuration with the given ID if present
    pub fn dna_by_id(&self, id: &str) -> Option<DnaConfig> {
        self.dnas.iter().find(|dc| dc.id == *id).cloned()
    }

    /// Returns the DNA configuration with the given ID if present
    pub fn update_dna_hash_by_id(&mut self, id: &str, hash: String) -> bool {
        self.dnas
            .iter_mut()
            .find(|dc| dc.id == *id)
            .map(|dna| dna.hash = hash)
            .is_some()
    }

    /// Returns the cell configuration with the given ID if present
    pub fn cell_by_id(&self, id: &str) -> Option<CellConfig> {
        self.cells.iter().find(|ic| ic.id == *id).cloned()
    }

    /// Returns the interface configuration with the given ID if present
    pub fn interface_by_id(&self, id: &str) -> Option<InterfaceConfig> {
        self.interfaces.iter().find(|ic| ic.id == *id).cloned()
    }

    /// Returns all defined cell IDs
    pub fn cell_ids(&self) -> Vec<String> {
        self.cells.iter().map(|cell| cell.id.clone()).collect()
    }

    /// Removes the cell given by id and all mentions of it in other elements so
    /// that the config is guaranteed to be valid afterwards if it was before.
    pub fn save_remove_cell(mut self, id: &str) -> Self {
        self.cells = self
            .cells
            .into_iter()
            .filter(|cell| cell.id != *id)
            .collect();

        self.interfaces = self
            .interfaces
            .into_iter()
            .map(|mut interface| {
                interface.cells = interface
                    .cells
                    .into_iter()
                    .filter(|cell| cell.id != *id)
                    .collect();
                interface
            })
            .collect();

        self
    }
}

/// An agent has a name/ID and is optionally defined by a private key that resides in a file
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub hash: AgentPubKey,
    pub keystore_file: String,
    /// If set to true conductor will ignore keystore_file and instead use the remote signer
    /// accessible through signing_service_uri to request signatures.
    pub holo_remote_key: Option<bool>,
    /// If true this agent will use dummy keys rather than a keystore file
    pub test_agent: Option<bool>,
}

/// A DNA is represented by a DNA file.
/// A hash can optionally be provided, which could be used to validate that the DNA being installed
/// is the DNA that was intended to be installed.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct DnaConfig {
    pub id: String,
    pub file: String,
    pub hash: String,
    #[serde(default)]
    pub uuid: Option<String>,
}

/// An cell combines a DNA with an agent.
/// Each cell has its own storage configuration.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct CellConfig {
    pub id: String,
    pub dna: String,
    pub agent: String,
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
/// An admin flag will enable conductor functions for programatically changing the configuration
/// (e.g. installing apps)
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct InterfaceConfig {
    pub id: String,
    pub driver: InterfaceDriver,
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub cells: Vec<CellReferenceConfig>,
    /// Experimental!
    /// If this flag is set the conductor might change the port the interface binds to if the
    /// given port is occupied. This might cause problems if the context that runs the conductor
    /// is not aware of this logic and is not tracking the new port (which gets printed on stdout).
    /// Use at your own risk...
    pub choose_free_port: Option<bool>,
}

/// An cell reference makes an cell available in the scope
/// of an interface.
/// Since UIs usually hard-code the name with which they reference an cell,
/// we need to decouple that name used by the UI from the internal ID of
/// the cell. That is what the optional `alias` field provides.
/// Given that there is 1-to-1 relationship between UIs and interfaces,
/// by setting an alias for available cells in the UI's interface
/// each UI can have its own unique handle for shared cells.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct CellReferenceConfig {
    /// ID of the cell that is made available in the interface
    pub id: String,

    /// A local name under which the cell gets mounted in the
    /// interface's scope
    pub alias: Option<String>,
}

/// A bridge enables an cell to call zome functions of another cell.
/// It is basically an internal interface.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct Bridge {
    /// ID of the cell that calls the other one.
    /// This cell depends on the callee.
    pub caller_id: String,

    /// ID of the cell that exposes traits through this bridge.
    /// This cell is used by the caller.
    pub callee_id: String,

    /// The caller's local handle of this bridge and the callee.
    /// A caller can have many bridges to other DNAs and those DNAs could
    /// by bound dynamically.
    /// Callers reference callees by this arbitrary but unique local name.
    pub handle: String,
}

// TODO: Tons of consistency check tests were ripped out in the great legacy code cleanup
// We need to add these back in when we've landed the new Dna format
// See https://github.com/Holo-Host/holochain-2020/blob/7750a0291e549be006529e4153b3b6cf0d686462/crates/holochain/src/conductor/state/tests.rs#L1
// for all old tests
