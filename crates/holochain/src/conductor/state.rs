use crate::conductor::error::ConductorError;
use boolinator::Boolinator;
use petgraph::{algo::toposort, graph::DiGraph, prelude::NodeIndex};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    fs::File,
    io::prelude::*,
    path::PathBuf,
    sync::Arc,
};
use sx_types::{
    agent::{AgentId, Base32},
    dna::{
        bridges::{BridgePresence, BridgeReference},
        Dna,
    },
    error::{SkunkError},
    prelude::*,
};
use toml;

/// Mutable conductor state, stored in a DB and writeable only via Admin interface.
///
/// References between structs (cell configs pointing to
/// the agent and DNA to be instantiated) are implemented
/// via string IDs.
#[derive(Deserialize, Serialize, Clone, Default, Debug)]
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

/// The default passphrase service is `Cmd` which will ask for a passphrase via stdout stdin.
/// In the context of a UI that wraps the conductor, this way of providing passphrases
/// is not feasible.
/// Setting the type to "unixsocket" and providing a path to a file socket enables
/// arbitrary UIs to connect to the conductor and prompt the user for a passphrase.
/// The according `PassphraseServiceUnixSocket` will send a request message over the socket
/// then receives bytes as passphrase until a newline is sent.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PassphraseServiceConfig {
    Cmd,
    UnixSocket { path: String },
    Mock { passphrase: String },
}

impl Default for PassphraseServiceConfig {
    fn default() -> PassphraseServiceConfig {
        PassphraseServiceConfig::Cmd
    }
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

        #[cfg(not(unix))]
        {
            if let PassphraseServiceConfig::UnixSocket { path } = self.passphrase_service.clone() {
                let _ = path;
                return Err(String::from(
                    "Passphrase service type 'unixsocket' is not available on non-Unix systems",
                ));
            }
        }

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
        let caller_config = self
            .cell_by_id(&bridge_config.caller_id)
            .ok_or_else(|| {
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
        let callee_config = self
            .cell_by_id(&bridge_config.callee_id)
            .ok_or_else(|| {
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
        self.cells
            .iter()
            .map(|cell| cell.id.clone())
            .collect()
    }

    /// This function uses the petgraph crate to model the bridge connections in this config
    /// as a graph and then create a topological sorting of the nodes, which are cells.
    /// The sorting gets reversed to get those cells first that do NOT depend on others
    /// such that this ordering of cells can be used to spawn them and simultaneously create
    /// initialize the bridges and be able to assert that any callee already exists (which makes
    /// this task much easier).
    pub fn cell_ids_sorted_by_bridge_dependencies(
        &self,
    ) -> Result<Vec<String>, ConductorError> {
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
        let edges: Vec<(&NodeIndex<u32>, &NodeIndex<u32>)> = self.bridges
            .iter()
            .map(|bridge| -> Result<(&NodeIndex<u32>, &NodeIndex<u32>), ConductorError> {
                let start = index_map.get(&bridge.caller_id);
                let end = index_map.get(&bridge.callee_id);
                if let (Some(start_inner), Some(end_inner)) = (start, end) {
                    Ok((start_inner, end_inner))
                }
                else {
                    Err(ConductorError::ConfigError(format!(
                        "cell configuration not found, mentioned in bridge configuration: {} -> {}",
                        bridge.caller_id, bridge.callee_id,
                    )))
                }
            })
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

impl From<AgentConfig> for AgentId {
    fn from(config: AgentConfig) -> Self {
        AgentId::try_from(JsonString::from_json(&config.id)).expect("bad agent json")
    }
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

impl TryFrom<DnaConfig> for Dna {
    type Error = SkunkError;
    fn try_from(dna_config: DnaConfig) -> Result<Self, Self::Error> {
        let mut f = File::open(dna_config.file)?;
        let mut contents = String::new();
        f.read_to_string(&mut contents)?;
        Dna::try_from(JsonString::from_json(&contents)).map_err(|err| err.into())
    }
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

// FIXME: there are a ton of tests here, pulled over from legacy code. They need to be refactored now that legacy Config has been split in two.
#[cfg(all(test, sx_refactor))]
pub mod tests {
    use super::*;
    use crate::config::{load_configuration, ConductorState, NetworkConfig};
    // use crate::test_fixtures::test_dna_loader;

    pub fn example_serialized_network_config() -> String {
        unimplemented!()
        // String::from(JsonString::from(P2pConfig::new_with_unique_memory_backend()))
    }

    #[test]
    fn test_agent_load() {
        let toml = r#"
    [[agents]]
    id = "bob"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "file/to/serialize"

    [[agents]]
    id="alex"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "another/file"

    [[dnas]]
    id="dna"
    file="file.dna.json"
    hash="QmDontCare"
    "#;
        let agents = load_configuration::<ConductorState>(toml).unwrap().agents;
        assert_eq!(agents.get(0).expect("expected at least 2 agents").id, "bob");
        assert_eq!(
            agents
                .get(0)
                .expect("expected at least 2 agents")
                .clone()
                .keystore_file,
            "file/to/serialize"
        );
        assert_eq!(
            agents.get(1).expect("expected at least 2 agents").id,
            "alex"
        );
    }

    #[test]
    fn test_dna_load() {
        let toml = r#"
    [[agents]]
    id="agent"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "whatever"

    [[dnas]]
    id = "app spec rust"
    file = "app_spec.dna.json"
    hash = "Qm328wyq38924y"
    "#;
        let dnas = load_configuration::<ConductorState>(toml).unwrap().dnas;
        let dna_config = dnas.get(0).expect("expected at least 1 DNA");
        assert_eq!(dna_config.id, "app spec rust");
        assert_eq!(dna_config.file, "app_spec.dna.json");
        assert_eq!(dna_config.hash, "Qm328wyq38924y".to_string());
    }

    #[test]
    fn test_load_complete_config() {
        let toml = r#"
    [[agents]]
    id = "test agent"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "holo_tester.key"

    [[dnas]]
    id = "app spec rust"
    file = "app_spec.dna.json"
    hash = "Qm328wyq38924y"

    [[cells]]
    id = "app spec cell"
    dna = "app spec rust"
    agent = "test agent"
        [cells.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec websocket interface"
        [interfaces.driver]
        type = "websocket"
        port = 8888
        [[interfaces.cells]]
        id = "app spec cell"

    [[interfaces]]
    id = "app spec http interface"
        [interfaces.driver]
        type = "http"
        port = 4000
        [[interfaces.cells]]
        id = "app spec cell"

    [[interfaces]]
    id = "app spec domainsocket interface"
        [interfaces.driver]
        type = "domainsocket"
        file = "/tmp/holochain.sock"
        [[interfaces.cells]]
        id = "app spec cell"

    [network]
    type = "sim2h"
    todo = "todo"

    [metric_publisher]
    type = "cloudwatchlogs"
    log_stream_name = "2019-11-22_20-53-31.sim2h_public"
    log_group_name = "holochain"

    "#;

        let config = load_configuration::<ConductorState>(toml).unwrap();

        assert_eq!(config.check_consistency(&mut test_dna_loader()), Ok(()));
        let dnas = config.dnas;
        let dna_config = dnas.get(0).expect("expected at least 1 DNA");
        assert_eq!(dna_config.id, "app spec rust");
        assert_eq!(dna_config.file, "app_spec.dna.json");
        assert_eq!(dna_config.hash, "Qm328wyq38924y".to_string());

        let cells = config.cells;
        let cell_config = cells.get(0).unwrap();
        assert_eq!(cell_config.id, "app spec cell");
        assert_eq!(cell_config.dna, "app spec rust");
        assert_eq!(cell_config.agent, "test agent");
        assert_eq!(config.logger.logger_level, "debug");
        // assert_eq!(format!("{:?}", config.metric_publisher), "Some(CloudWatchLogs(CloudWatchLogsConfig { region: None, log_group_name: Some(\"holochain\"), log_stream_name: Some(\"2019-11-22_20-53-31.sim2h_public\"), assume_role_arn: None }))");
        // assert_eq!(
        //     config.network.unwrap(),
        //     NetworkConfig::N3h(N3hConfig {
        //         bootstrap_nodes: vec![String::from(
        //             "wss://192.168.0.11:64519/?a=hkYW7TrZUS1hy-i374iRu5VbZP1sSw2mLxP4TSe_YI1H2BJM3v_LgAQnpmWA_iR1W5k-8_UoA1BNjzBSUTVNDSIcz9UG0uaM"
        //         )],
        //         n3h_log_level: String::from("d"),
        //         n3h_mode: String::from("REAL"),
        //         n3h_persistence_path: String::from("/Users/cnorris/.holochain/n3h_persistence"),
        //         n3h_ipc_uri: None,
        //         networking_config_file: Some(String::from(
        //             "/Users/cnorris/.holochain/network_config.json"
        //         )),
        //     })
        // );
    }

    #[test]
    fn test_load_complete_config_default_network() {
        let toml = r#"
    [[agents]]
    id = "test agent"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "holo_tester.key"

    [[dnas]]
    id = "app spec rust"
    file = "app_spec.dna.json"
    hash = "Qm328wyq38924y"

    [[cells]]
    id = "app spec cell"
    dna = "app spec rust"
    agent = "test agent"
        [cells.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec websocket interface"
        [interfaces.driver]
        type = "websocket"
        port = 8888
        [[interfaces.cells]]
        id = "app spec cell"

    [[interfaces]]
    id = "app spec http interface"
        [interfaces.driver]
        type = "http"
        port = 4000
        [[interfaces.cells]]
        id = "app spec cell"

    [[interfaces]]
    id = "app spec domainsocket interface"
        [interfaces.driver]
        type = "domainsocket"
        file = "/tmp/holochain.sock"
        [[interfaces.cells]]
        id = "app spec cell"

    [logger]
    type = "debug"
        [[logger.rules.rules]]
        pattern = ".*"
        color = "red"

    [[ui_bundles]]
    id = "bundle1"
    root_dir = "" # serves the current directory
    hash = "Qm000"

    [[ui_interfaces]]
    id = "ui-interface-1"
    bundle = "bundle1"
    port = 3000
    dna_interface = "app spec domainsocket interface"
    "#;

        let config = load_configuration::<ConductorState>(toml).unwrap();

        assert_eq!(config.check_consistency(&mut test_dna_loader()), Ok(()));
        let dnas = config.dnas;
        let dna_config = dnas.get(0).expect("expected at least 1 DNA");
        assert_eq!(dna_config.id, "app spec rust");
        assert_eq!(dna_config.file, "app_spec.dna.json");
        assert_eq!(dna_config.hash, "Qm328wyq38924y".to_string());

        let cells = config.cells;
        let cell_config = cells.get(0).unwrap();
        assert_eq!(cell_config.id, "app spec cell");
        assert_eq!(cell_config.dna, "app spec rust");
        assert_eq!(cell_config.agent, "test agent");
        assert_eq!(config.logger.logger_level, "debug");
        // assert_eq!(config.logger.rules.rules.len(), 1);

        assert_eq!(config.network, None);
    }

    #[test]
    fn test_load_bad_network_config() {
        let base_toml = r#"
    [[agents]]
    id = "test agent"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "holo_tester.key"

    [[dnas]]
    id = "app spec rust"
    file = "app_spec.dna.json"
    hash = "Qm328wyq38924y"

    [[cells]]
    id = "app spec cell"
    dna = "app spec rust"
    agent = "test agent"
        [cells.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec websocket interface"
        [interfaces.driver]
        type = "websocket"
        port = 8888
        [[interfaces.cells]]
        id = "app spec cell"
    "#;

        let toml = format!(
            "{}{}",
            base_toml,
            r#"
    [network]
    type = "lib3h"
    "#
        );
        if let Err(e) = load_configuration::<ConductorState>(toml.as_str()) {
            assert!(
                true,
                e.to_string().contains(
                    "Error loading configuration: missing field `socket_type` for key `network`"
                )
            )
        } else {
            panic!("Should have failed!")
        }
    }

    #[test]
    fn test_inconsistent_config() {
        let toml = r#"
    [[agents]]
    id = "test agent"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "holo_tester.key"

    [[dnas]]
    id = "app spec rust"
    file = "app_spec.dna.json"
    hash = "Qm328wyq38924y"

    [[cells]]
    id = "app spec cell"
    dna = "WRONG DNA ID"
    agent = "test agent"
        [cells.storage]
        type = "file"
        path = "app_spec_storage"
    "#;

        let config: ConductorState =
            load_configuration(toml).expect("Failed to load config from toml string");

        assert_eq!(config.check_consistency(&mut test_dna_loader()), Err("DNA configuration \"WRONG DNA ID\" not found, mentioned in cell \"app spec cell\"".to_string().into()));
    }

    #[test]
    fn test_inconsistent_config_interface_1() {
        let toml = r#"
    [[agents]]
    id = "test agent"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "holo_tester.key"

    [[dnas]]
    id = "app spec rust"
    file = "app_spec.dna.json"
    hash = "Qm328wyq38924y"

    [[cells]]
    id = "app spec cell"
    dna = "app spec rust"
    agent = "test agent"
        [cells.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec interface"
        [interfaces.driver]
        type = "websocket"
        port = 8888
        [[interfaces.cells]]
        id = "WRONG cell ID"
    "#;

        let config = load_configuration::<ConductorState>(toml).unwrap();

        assert_eq!(
            config.check_consistency(&mut test_dna_loader()),
            Err(
                "cell configuration \"WRONG cell ID\" not found, mentioned in interface"
                    .to_string()
                    .into()
            )
        );
    }

    #[test]
    fn test_invalid_toml_1() {
        let toml = &format!(
            r#"
    [[agents]]
    id = "test agent"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "holo_tester.key"

    [[dnas]]
    id = "app spec rust"
    file = "app-spec-rust.dna.json"
    hash = "Qm328wyq38924y"

    [[cells]]
    id = "app spec cell"
    dna = "app spec rust"
    agent = "test agent"
    network = "{}"
        [cells.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec interface"
        [interfaces.driver]
        type = "invalid type"
        port = 8888
        [[interfaces.cells]]
        id = "app spec cell"
    "#,
            example_serialized_network_config()
        );
        if let Err(e) = load_configuration::<ConductorState>(toml) {
            assert!(
                true,
                e.to_string().contains("unknown variant `invalid type`")
            )
        } else {
            panic!("Should have failed!")
        }
    }

    fn bridges_config(bridges: &str) -> String {
        format!(
            r#"
    [[agents]]
    id = "test agent"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "holo_tester.key"

    [[dnas]]
    id = "bridge caller"
    file = "bridge/caller_without_required.dna"
    hash = "Qm328wyq38924y"

    [[cells]]
    id = "app1"
    dna = "bridge caller"
    agent = "test agent"
        [cells.storage]
        type = "file"
        path = "app1_spec_storage"

    [[cells]]
    id = "app2"
    dna = "bridge caller"
    agent = "test agent"
        [cells.storage]
        type = "file"
        path = "app2_spec_storage"

    [[cells]]
    id = "app3"
    dna = "bridge caller"
    agent = "test agent"
        [cells.storage]
        type = "file"
        path = "app3_spec_storage"

    {}
    "#,
            bridges
        )
    }

    #[test]
    fn test_bridge_config() {
        let toml = bridges_config(
            r#"
    [[bridges]]
    caller_id = "app1"
    callee_id = "app2"
    handle = "happ-store"

    [[bridges]]
    caller_id = "app2"
    callee_id = "app3"
    handle = "DPKI"
    "#,
        );
        let config =
            load_configuration::<ConductorState>(&toml).expect("ConductorState should be syntactically correct");
        assert_eq!(config.check_consistency(&mut test_dna_loader()), Ok(()));

        // "->": calls
        // app1 -> app2 -> app3
        // app3 has no dependency so it can be instantiated first.
        // app2 depends on (calls) only app3, so app2 is next.
        // app1 should be last.
        assert_eq!(
            config.cell_ids_sorted_by_bridge_dependencies(),
            Ok(vec![
                String::from("app3"),
                String::from("app2"),
                String::from("app1")
            ])
        );
    }

    #[test]
    fn test_bridge_cycle() {
        let toml = bridges_config(
            r#"
    [[bridges]]
    caller_id = "app1"
    callee_id = "app2"
    handle = "happ-store"

    [[bridges]]
    caller_id = "app2"
    callee_id = "app3"
    handle = "DPKI"

    [[bridges]]
    caller_id = "app3"
    callee_id = "app1"
    handle = "test-callee"
    "#,
        );
        let config =
            load_configuration::<ConductorState>(&toml).expect("ConductorState should be syntactically correct");
        assert_eq!(
            config.check_consistency(&mut test_dna_loader()),
            Err("Cyclic dependency in bridge configuration"
                .to_string()
                .into())
        );
    }

    #[test]
    fn test_bridge_non_existent() {
        let toml = bridges_config(
            r#"
    [[bridges]]
    caller_id = "app1"
    callee_id = "app2"
    handle = "happ-store"

    [[bridges]]
    caller_id = "app2"
    callee_id = "app3"
    handle = "DPKI"

    [[bridges]]
    caller_id = "app9000"
    callee_id = "app1"
    handle = "something"
    "#,
        );
        let config =
            load_configuration::<ConductorState>(&toml).expect("ConductorState should be syntactically correct");
        assert_eq!(
            config.check_consistency(&mut test_dna_loader()),
            Err(
                "cell configuration \"app9000\" not found, mentioned in bridge"
                    .to_string()
                    .into()
            )
        );
    }

    #[test]
    fn test_bridge_dependencies() {
        let toml = bridges_config(
            r#"
    [[bridges]]
    caller_id = "app1"
    callee_id = "app2"
    handle = "happ-store"

    [[bridges]]
    caller_id = "app1"
    callee_id = "app3"
    handle = "happ-store"

    [[bridges]]
    caller_id = "app2"
    callee_id = "app1"
    handle = "happ-store"
    "#,
        );
        let config =
            load_configuration::<ConductorState>(&toml).expect("ConductorState should be syntactically correct");
        let bridged_ids: Vec<_> = config
            .bridge_dependencies(String::from("app1"))
            .iter()
            .map(|bridge| bridge.callee_id.clone())
            .collect();
        assert_eq!(
            bridged_ids,
            vec![String::from("app2"), String::from("app3"),]
        );
    }

    #[test]
    fn test_inconsistent_ui_interface() {
        let toml = r#"
    [[agents]]
    id = "test agent"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "holo_tester.key"

    [[dnas]]
    id = "app spec rust"
    file = "app_spec.dna.json"
    hash = "Qm328wyq38924y"

    [[cells]]
    id = "app spec cell"
    dna = "app spec rust"
    agent = "test agent"
        [cells.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec websocket interface"
        [interfaces.driver]
        type = "websocket"
        port = 8888
        [[interfaces.cells]]
        id = "app spec cell"

    [[interfaces]]
    id = "app spec http interface"
        [interfaces.driver]
        type = "http"
        port = 4000
        [[interfaces.cells]]
        id = "app spec cell"

    [[interfaces]]
    id = "app spec domainsocket interface"
        [interfaces.driver]
        type = "domainsocket"
        file = "/tmp/holochain.sock"
        [[interfaces.cells]]
        id = "app spec cell"

    [logger]
    type = "debug"
        [[logger.rules.rules]]
        pattern = ".*"
        color = "red"

    [[ui_bundles]]
    id = "bundle1"
    root_dir = "" # serves the current directory
    hash = "Qm000"

    [[ui_interfaces]]
    id = "ui-interface-1"
    bundle = "bundle1"
    port = 3000
    dna_interface = "<not existant>"
    "#;
        let config =
            load_configuration::<ConductorState>(&toml).expect("ConductorState should be syntactically correct");
        assert_eq!(
            config.check_consistency(&mut test_dna_loader()),
            Err("DNA Interface configuration \"<not existant>\" not found, mentioned in UI interface \"ui-interface-1\"".to_string().into())
        );
    }

    #[test]
    fn test_inconsistent_dpki() {
        let toml = r#"
    [[agents]]
    id = "test agent"
    name = "Holo Tester 1"
    public_address = "HoloTester1-------------------------------------------------------------------------AHi1"
    keystore_file = "holo_tester.key"

    [[dnas]]
    id = "deepkey"
    file = "deepkey.dna.json"
    hash = "Qm328wyq38924y"

    [[cells]]
    id = "deepkey"
    dna = "deepkey"
    agent = "test agent"
        [cells.storage]
        type = "file"
        path = "deepkey_storage"

    [dpki]
    cell_id = "bogus cell"
    init_params = "{}"
    "#;
        let config =
            load_configuration::<ConductorState>(&toml).expect("ConductorState should be syntactically correct");
        assert_eq!(
            config.check_consistency(&mut test_dna_loader()),
            Err(
                "cell configuration \"bogus cell\" not found, mentioned in dpki"
                    .to_string()
                    .into()
            )
        );
    }

    #[test]
    fn test_check_cells_storage() -> Result<(), String> {
        let toml = r#"
        [[agents]]
        id = "test agent 1"
        keystore_file = "holo_tester.key"
        name = "Holo Tester 1"
        public_address = "HoloTester1-----------------------------------------------------------------------AAACZp4xHB"

        [[agents]]
        id = "test agent 2"
        keystore_file = "holo_tester.key"
        name = "Holo Tester 2"
        public_address = "HoloTester2-----------------------------------------------------------------------AAAGy4WW9e"

        [[cells]]
        agent = "test agent 1"
        dna = "app spec rust"
        id = "app spec cell 1"

            [cells.storage]
            path = "example-config/tmp-storage-1"
            type = "file"

        [[cells]]
        agent = "test agent 2"
        dna = "app spec rust"
        id = "app spec cell 2"

            [cells.storage]
            path = "example-config/tmp-storage-2"
            type = "file"
        "#;

        let config =
            load_configuration::<ConductorState>(&toml).expect("ConductorState should be syntactically correct");

        assert_eq!(config.check_cells_storage(), Ok(()));
        Ok(())
    }

    #[test]
    fn test_check_cells_storage_err() -> Result<(), String> {
        // Here we have a forbidden duplicated 'cells.storage'
        let toml = r#"
        [[agents]]
        id = "test agent 1"
        keystore_file = "holo_tester.key"
        name = "Holo Tester 1"
        public_address = "HoloTester1-----------------------------------------------------------------------AAACZp4xHB"

        [[cells]]
        agent = "test agent 1"
        dna = "app spec rust"
        id = "app spec cell 1"

            [cells.storage]
            path = "forbidden-duplicated-storage-file-path"
            type = "file"

        [[cells]]
        agent = "test agent 2"
        dna = "app spec rust"
        id = "app spec cell 2"

            [cells.storage]
            path = "forbidden-duplicated-storage-file-path"
            type = "file"
        "#;

        let config =
            load_configuration::<ConductorState>(&toml).expect("ConductorState should be syntactically correct");

        assert_eq!(
            config.check_cells_storage(),
            Err(String::from(
                "Forbidden duplicated file storage value encountered."
            ))
        );
        Ok(())
    }
}
