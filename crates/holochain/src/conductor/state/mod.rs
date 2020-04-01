use crate::conductor::error::ConductorError;
use boolinator::Boolinator;
use petgraph::{algo::toposort, graph::DiGraph, prelude::NodeIndex};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use sx_types::{
    agent::{AgentId, Base32},
    dna::{
        bridges::{BridgePresence, BridgeReference},
        Dna,
    },
    error::SkunkError,
    prelude::*,
};
use toml;

#[cfg(test)]
mod tests;

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

    /// List of bridges between cells. Optional.
    #[serde(default)]
    pub bridges: Vec<Bridge>,
}

/// Check for duplicate items in a list of strings
fn detect_dupes<'a, I: Iterator<Item = &'a String>>(
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

pub type DnaLoader = Arc<Box<dyn FnMut(&PathBuf) -> Result<Dna, SkunkError> + Send + Sync>>;

impl ConductorState {
    /// This function basically checks if self is a semantically valid configuration.
    /// This mainly means checking for consistency between config structs that reference others.
    /// FIXME: this function was ported over from legacy code, and then parts were ripped out until it compiled.
    ///     this is because we need the ConductorConfig to do some of these consistency checks.
    ///     If we keep using this representation of ConductorState, then add them back in.
    pub fn check_consistency(&self, mut dna_loader: &mut DnaLoader) -> Result<(), ConductorError> {
        detect_dupes("agent", self.agents.iter().map(|c| &c.id))?;
        detect_dupes("dna", self.dnas.iter().map(|c| &c.id))?;

        detect_dupes("cell", self.cells.iter().map(|c| &c.id))?;

        detect_dupes("interface", self.interfaces.iter().map(|c| &c.id))?;

        for ref cell in self.cells.iter() {
            self.agent_by_id(&cell.agent).is_some().ok_or_else(|| {
                format!(
                    "Agent configuration {} not found, mentioned in cell {}",
                    cell.agent, cell.id
                )
            })?;
            let dna_config = self.dna_by_id(&cell.dna);
            dna_config.is_some().ok_or_else(|| {
                format!(
                    "DNA configuration \"{}\" not found, mentioned in cell \"{}\"",
                    cell.dna, cell.id
                )
            })?;
            let dna_config = dna_config.unwrap();
            let dna =
                Arc::get_mut(&mut dna_loader).unwrap()(&PathBuf::from(dna_config.file.clone()))
                    .map_err(|_| format!("Could not load DNA file \"{}\"", dna_config.file))?;

            for zome in dna.zomes.values() {
                for bridge in zome.bridges.iter() {
                    if bridge.presence == BridgePresence::Required {
                        let handle = bridge.handle.clone();
                        let _ = self
                            .bridges
                            .iter()
                            .find(|b| b.handle == handle)
                            .ok_or_else(|| {
                                format!(
                                    "Required bridge '{}' for cell '{}' missing",
                                    handle, cell.id
                                )
                            })?;
                    }
                }
            }
        }

        for ref interface in self.interfaces.iter() {
            for ref cell in interface.cells.iter() {
                self.cell_by_id(&cell.id).is_some().ok_or_else(|| {
                    format!(
                        "cell configuration \"{}\" not found, mentioned in interface",
                        cell.id
                    )
                })?;
            }
        }

        for bridge in self.bridges.iter() {
            self.check_bridge_requirements(bridge, dna_loader)?;
        }

        let _ = self.cell_ids_sorted_by_bridge_dependencies()?;

        Ok(())
    }

    fn check_bridge_requirements(
        &self,
        bridge_config: &Bridge,
        mut dna_loader: &mut DnaLoader,
    ) -> Result<(), String> {
        //
        // Get caller's config. DNA config, and DNA:
        //
        let caller_config = self.cell_by_id(&bridge_config.caller_id).ok_or_else(|| {
            format!(
                "cell configuration \"{}\" not found, mentioned in bridge",
                bridge_config.caller_id
            )
        })?;

        let caller_dna_config = self.dna_by_id(&caller_config.dna).ok_or_else(|| {
            format!(
                "DNA configuration \"{}\" not found, mentioned in cell \"{}\"",
                caller_config.dna, caller_config.id
            )
        })?;

        let caller_dna_file = caller_dna_config.file;
        let caller_dna =
            Arc::get_mut(&mut dna_loader).unwrap()(&PathBuf::from(caller_dna_file.clone()))
                .map_err(|err| {
                    format!(
                        "Could not load DNA file \"{}\"; error was: {}",
                        caller_dna_file, err
                    )
                })?;

        //
        // Get callee's config. DNA config, and DNA:
        //
        let callee_config = self.cell_by_id(&bridge_config.callee_id).ok_or_else(|| {
            format!(
                "cell configuration \"{}\" not found, mentioned in bridge",
                bridge_config.callee_id
            )
        })?;

        let callee_dna_config = self.dna_by_id(&callee_config.dna).ok_or_else(|| {
            format!(
                "DNA configuration \"{}\" not found, mentioned in cell \"{}\"",
                callee_config.dna, callee_config.id
            )
        })?;

        let callee_dna_file = callee_dna_config.file;
        let callee_dna =
            Arc::get_mut(&mut dna_loader).unwrap()(&PathBuf::from(callee_dna_file.clone()))
                .map_err(|err| {
                    format!(
                        "Could not load DNA file \"{}\"; error was: {}",
                        callee_dna_file, err
                    )
                })?;

        //
        // Get matching bridge definition from caller's DNA:
        //
        let mut maybe_bridge = None;
        for zome in caller_dna.zomes.values() {
            for bridge_def in zome.bridges.iter() {
                if bridge_def.handle == bridge_config.handle {
                    maybe_bridge = Some(bridge_def.clone());
                }
            }
        }

        let bridge = maybe_bridge.ok_or_else(|| {
            format!(
                "No bridge definition with handle '{}' found in {}'s DNA",
                bridge_config.handle, bridge_config.caller_id,
            )
        })?;

        match bridge.reference {
            BridgeReference::Address { ref dna_address } => {
                if *dna_address != callee_dna.address() {
                    return Err(format!(
                        "Bridge '{}' of caller cell '{}' requires callee to be DNA with hash '{}', but the configured cell '{}' runs DNA with hash '{}'.",
                        bridge.handle,
                        bridge_config.caller_id,
                        dna_address,
                        callee_config.id,
                        callee_dna.address(),
                    ));
                }
            }
            BridgeReference::Trait { ref traits } => {
                for (expected_trait_name, expected_trait) in traits {
                    let mut found = false;
                    for (_zome_name, zome) in callee_dna.zomes.iter() {
                        for (zome_trait_name, zome_trait_functions) in zome.traits.iter() {
                            if zome_trait_name == expected_trait_name {
                                let mut has_all_fns_exported = true;
                                for fn_def in expected_trait.functions.iter() {
                                    if !zome_trait_functions.functions.contains(&fn_def.name) {
                                        has_all_fns_exported = false;
                                    }
                                }

                                let mut has_matching_signatures = true;
                                if has_all_fns_exported {
                                    for fn_def in expected_trait.functions.iter() {
                                        if !zome.fn_declarations.contains(&fn_def) {
                                            has_matching_signatures = false;
                                        }
                                    }
                                }

                                if has_all_fns_exported && has_matching_signatures {
                                    found = true;
                                }
                            }
                        }
                    }

                    if !found {
                        return Err(format!(
                            "Bridge '{}' of cell '{}' requires callee to to implement trait '{}' with functions: {:?}",
                            bridge.handle,
                            bridge_config.caller_id,
                            expected_trait_name,
                            expected_trait.functions,
                        ));
                    }
                }
            }
        };
        Ok(())
    }

    /// Returns the agent configuration with the given ID if present
    pub fn agent_by_id(&self, id: &str) -> Option<AgentConfig> {
        self.agents.iter().find(|ac| &ac.id == id).cloned()
    }

    /// Returns the agent configuration with the given ID if present
    pub fn update_agent_address_by_id(&mut self, id: &str, agent_id: &AgentId) {
        self.agents.iter_mut().for_each(|ac| {
            if &ac.id == id {
                ac.public_address = agent_id.pub_sign_key().clone()
            }
        })
    }

    /// Returns the DNA configuration with the given ID if present
    pub fn dna_by_id(&self, id: &str) -> Option<DnaConfig> {
        self.dnas.iter().find(|dc| &dc.id == id).cloned()
    }

    /// Returns the DNA configuration with the given ID if present
    pub fn update_dna_hash_by_id(&mut self, id: &str, hash: String) -> bool {
        self.dnas
            .iter_mut()
            .find(|dc| &dc.id == id)
            .map(|dna| dna.hash = hash)
            .is_some()
    }

    /// Returns the cell configuration with the given ID if present
    pub fn cell_by_id(&self, id: &str) -> Option<CellConfig> {
        self.cells.iter().find(|ic| &ic.id == id).cloned()
    }

    /// Returns the interface configuration with the given ID if present
    pub fn interface_by_id(&self, id: &str) -> Option<InterfaceConfig> {
        self.interfaces.iter().find(|ic| &ic.id == id).cloned()
    }

    /// Returns all defined cell IDs
    pub fn cell_ids(&self) -> Vec<String> {
        self.cells.iter().map(|cell| cell.id.clone()).collect()
    }

    /// This function uses the petgraph crate to model the bridge connections in this config
    /// as a graph and then create a topological sorting of the nodes, which are cells.
    /// The sorting gets reversed to get those cells first that do NOT depend on others
    /// such that this ordering of cells can be used to spawn them and simultaneously create
    /// initialize the bridges and be able to assert that any callee already exists (which makes
    /// this task much easier).
    pub fn cell_ids_sorted_by_bridge_dependencies(&self) -> Result<Vec<String>, ConductorError> {
        let mut graph = DiGraph::<&str, &str>::new();

        // Add cell ids to the graph which returns the indices the graph is using.
        // Storing those in a map from ids to create edges from bridges below.
        let index_map: HashMap<_, _> = self
            .cells
            .iter()
            .map(|cell| (cell.id.clone(), graph.add_node(&cell.id)))
            .collect();

        // Reverse of graph indices to cell ids to create the return vector below.
        let reverse_map: HashMap<_, _> = self
            .cells
            .iter()
            .map(|cell| (index_map.get(&cell.id).unwrap(), cell.id.clone()))
            .collect();

        // Create vector of edges (with node indices) from bridges:
        let edges: Vec<(&NodeIndex<u32>, &NodeIndex<u32>)> = self
            .bridges
            .iter()
            .map(
                |bridge| -> Result<(&NodeIndex<u32>, &NodeIndex<u32>), ConductorError> {
                    let start = index_map.get(&bridge.caller_id);
                    let end = index_map.get(&bridge.callee_id);
                    if let (Some(start_inner), Some(end_inner)) = (start, end) {
                        Ok((start_inner, end_inner))
                    } else {
                        Err(ConductorError::ConfigError(format!(
                        "cell configuration not found, mentioned in bridge configuration: {} -> {}",
                        bridge.caller_id, bridge.callee_id,
                    )))
                    }
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        // Add edges to graph:
        for &(node_a, node_b) in edges.iter() {
            graph.add_edge(node_a.clone(), node_b.clone(), "");
        }

        // Sort with petgraph::algo::toposort
        let mut sorted_nodes = toposort(&graph, None).map_err(|_cycle_error| {
            ConductorError::ConfigError("Cyclic dependency in bridge configuration".to_string())
        })?;

        // REVERSE order because we want to get the cell with NO dependencies first
        // since that is the cell we should spawn first.
        sorted_nodes.reverse();

        // Map sorted vector of node indices back to cell ids
        Ok(sorted_nodes
            .iter()
            .map(|node_index| reverse_map.get(node_index).unwrap())
            .cloned()
            .collect())
    }

    pub fn bridge_dependencies(&self, caller_cell_id: String) -> Vec<Bridge> {
        self.bridges
            .iter()
            .filter(|bridge| bridge.caller_id == caller_cell_id)
            .cloned()
            .collect()
    }

    /// Removes the cell given by id and all mentions of it in other elements so
    /// that the config is guaranteed to be valid afterwards if it was before.
    pub fn save_remove_cell(mut self, id: &String) -> Self {
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
    pub public_address: Base32,
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

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InterfaceDriver {
    Websocket { port: u16 },
    Http { port: u16 },
    DomainSocket { file: String },
    Custom(toml::value::Value),
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
