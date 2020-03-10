use crate::conductor::error::ConductorError;
use boolinator::Boolinator;
use petgraph::{algo::toposort, graph::DiGraph, prelude::NodeIndex};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    env,
    fs::File,
    io::prelude::*,
    net::Ipv4Addr,
    path::PathBuf,
    sync::Arc,
};
use sx_types::{
    agent::{AgentId, Base32},
    dna::{
        bridges::{BridgePresence, BridgeReference},
        Dna,
    },
    error::{SkunkError, SkunkResult},
    prelude::*,
    shims::*,
};
use toml;
// use crate::{conductor::base::DnaLoader, logger::LogRules};
// use holochain_metrics::MetricPublisherConfig;

/// Main conductor configuration struct
/// This is the root of the configuration tree / aggregates
/// all other configuration aspects.
///
/// References between structs (instance configs pointing to
/// the agent and DNA to be instantiated) are implemented
/// via string IDs.
#[derive(Deserialize, Serialize, Clone, Default, Debug)]
pub struct Config {
    /// List of Agents, this mainly means identities and their keys. Required.
    pub agents: Vec<AgentConfig>,
    /// List of DNAs, for each a path to the DNA file. Optional.
    #[serde(default)]
    pub dnas: Vec<DnaConfig>,
    /// List of instances, includes references to an agent and a DNA. Optional.
    #[serde(default)]
    pub instances: Vec<InstanceConfig>,
    /// List of interfaces any UI can use to access zome functions. Optional.
    #[serde(default)]
    pub interfaces: Vec<InterfaceConfig>,

    /// List of bridges between instances. Optional.
    #[serde(default)]
    pub bridges: Vec<Bridge>,

    // /// Configures how logging should behave. Optional.
    // #[serde(default)]
    // pub logger: LoggerConfig,
    /// Config options for the network module. Optional.
    #[serde(default)]
    pub network: Option<NetworkConfig>,
    /// where to persist the config file and DNAs. Optional.
    #[serde(default = "default_persistence_dir")]
    pub persistence_dir: PathBuf,

    /// Optional URI for a websocket connection to an outsourced signing service.
    /// Bootstrapping step for Holo closed-alpha.
    /// If set, all agents with holo_remote_key = true will be emulated by asking for signatures
    /// over this websocket.
    pub signing_service_uri: Option<String>,

    /// Optional URI for a websocket connection to an outsourced encryption service.
    /// Bootstrapping step for Holo closed-alpha.
    /// If set, all agents with holo_remote_key = true will be emulated by asking for signatures
    /// over this websocket.
    pub encryption_service_uri: Option<String>,

    /// Optional URI for a websocket connection to an outsourced decryption service.
    /// Bootstrapping step for Holo closed-alpha.
    /// If set, all agents with holo_remote_key = true will be emulated by asking for signatures
    /// over this websocket.
    pub decryption_service_uri: Option<String>,

    /// Optional DPKI configuration if conductor is using a DPKI app to initalize and manage
    /// keys for new instances
    pub dpki: Option<DpkiConfig>,

    /// Which signals to emit
    #[serde(default)]
    pub signals: SignalConfig,

    /// Configure how the conductor should prompt the user for the passphrase to lock/unlock keystores.
    /// The conductor is independent of the specialized implementation of the trait
    /// PassphraseService. It just needs something to provide a passphrase when needed.
    /// This config setting selects one of the available services (i.e. CLI prompt, IPC, mock)
    #[serde(default)]
    pub passphrase_service: PassphraseServiceConfig,
    // TODO: can't hook up until holochain_metrics has a new version pushed with bump to lazy_static dep
    // #[serde(default)]
    // pub metric_publisher: Option<MetricPublisherConfig>,
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

pub fn default_persistence_dir() -> PathBuf {
    holochain_common::paths::config_root().join("conductor")
}

// /// This is a config helper structure used to interface with the holochain logging subcrate.
// /// Custom rules/filter can be applied to logging, in fact they are used by default in Holochain to
// /// filter the logs from its dependencies.
// ///
// /// ```rust
// /// extern crate sx_conductor_lib;
// /// use sx_conductor_lib::{logger, config};
// /// let mut rules = logger::LogRules::new();
// /// // Filtering out all the logs from our dependencies
// /// rules
// ///     .add_rule(".*", true, None)
// ///     .expect("Invalid logging rule.");
// /// // And logging back all Holochain logs
// /// rules
// ///     .add_rule("^holochain", false, None)
// ///     .expect("Invalid logging rule.");
// ///
// /// let lc = config::LoggerConfig {
// ///     logger_level: "debug".to_string(),
// ///     rules: rules,
// ///     state_dump: true,
// ///     };
// /// ```
// #[derive(Deserialize, Serialize, Clone, Debug)]
// pub struct LoggerConfig {
//     #[serde(rename = "type")]
//     pub logger_level: String,
//     #[serde(default)]
//     pub rules: LogRules,
//     //    pub file: Option<String>,
//     #[serde(default)]
//     pub state_dump: bool,
// }

// impl Default for LoggerConfig {
//     fn default() -> LoggerConfig {
//         LoggerConfig {
//             logger_level: "debug".into(),
//             rules: Default::default(),
//             state_dump: false,
//         }
//     }
// }

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

// #[holochain_tracing_macros::newrelic_autotrace(HOLOCHAIN_CONDUCTOR_LIB)]
impl Config {
    /// This function basically checks if self is a semantically valid configuration.
    /// This mainly means checking for consistency between config structs that reference others.
    pub fn check_consistency(&self, mut dna_loader: &mut DnaLoader) -> Result<(), ConductorError> {
        detect_dupes("agent", self.agents.iter().map(|c| &c.id))?;
        detect_dupes("dna", self.dnas.iter().map(|c| &c.id))?;

        detect_dupes("instance", self.instances.iter().map(|c| &c.id))?;
        self.check_instances_storage()?;

        detect_dupes("interface", self.interfaces.iter().map(|c| &c.id))?;

        for ref instance in self.instances.iter() {
            self.agent_by_id(&instance.agent).is_some().ok_or_else(|| {
                format!(
                    "Agent configuration {} not found, mentioned in instance {}",
                    instance.agent, instance.id
                )
            })?;
            let dna_config = self.dna_by_id(&instance.dna);
            dna_config.is_some().ok_or_else(|| {
                format!(
                    "DNA configuration \"{}\" not found, mentioned in instance \"{}\"",
                    instance.dna, instance.id
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
                                    "Required bridge '{}' for instance '{}' missing",
                                    handle, instance.id
                                )
                            })?;
                    }
                }
            }
        }

        for ref interface in self.interfaces.iter() {
            for ref instance in interface.instances.iter() {
                self.instance_by_id(&instance.id).is_some().ok_or_else(|| {
                    format!(
                        "Instance configuration \"{}\" not found, mentioned in interface",
                        instance.id
                    )
                })?;
            }
        }

        for bridge in self.bridges.iter() {
            self.check_bridge_requirements(bridge, dna_loader)?;
        }

        if let Some(ref dpki_config) = self.dpki {
            self.instance_by_id(&dpki_config.instance_id)
                .is_some()
                .ok_or_else(|| {
                    format!(
                        "Instance configuration \"{}\" not found, mentioned in dpki",
                        dpki_config.instance_id
                    )
                })?;
        }

        let _ = self.instance_ids_sorted_by_bridge_dependencies()?;

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
            .instance_by_id(&bridge_config.caller_id)
            .ok_or_else(|| {
                format!(
                    "Instance configuration \"{}\" not found, mentioned in bridge",
                    bridge_config.caller_id
                )
            })?;

        let caller_dna_config = self.dna_by_id(&caller_config.dna).ok_or_else(|| {
            format!(
                "DNA configuration \"{}\" not found, mentioned in instance \"{}\"",
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
            .instance_by_id(&bridge_config.callee_id)
            .ok_or_else(|| {
                format!(
                    "Instance configuration \"{}\" not found, mentioned in bridge",
                    bridge_config.callee_id
                )
            })?;

        let callee_dna_config = self.dna_by_id(&callee_config.dna).ok_or_else(|| {
            format!(
                "DNA configuration \"{}\" not found, mentioned in instance \"{}\"",
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
                        "Bridge '{}' of caller instance '{}' requires callee to be DNA with hash '{}', but the configured instance '{}' runs DNA with hash '{}'.",
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
                            "Bridge '{}' of instance '{}' requires callee to to implement trait '{}' with functions: {:?}",
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

    /// Returns the instance configuration with the given ID if present
    pub fn instance_by_id(&self, id: &str) -> Option<InstanceConfig> {
        self.instances.iter().find(|ic| &ic.id == id).cloned()
    }

    /// Returns the interface configuration with the given ID if present
    pub fn interface_by_id(&self, id: &str) -> Option<InterfaceConfig> {
        self.interfaces.iter().find(|ic| &ic.id == id).cloned()
    }

    /// Returns all defined instance IDs
    pub fn instance_ids(&self) -> Vec<String> {
        self.instances
            .iter()
            .map(|instance| instance.id.clone())
            .collect()
    }

    /// This function uses the petgraph crate to model the bridge connections in this config
    /// as a graph and then create a topological sorting of the nodes, which are instances.
    /// The sorting gets reversed to get those instances first that do NOT depend on others
    /// such that this ordering of instances can be used to spawn them and simultaneously create
    /// initialize the bridges and be able to assert that any callee already exists (which makes
    /// this task much easier).
    pub fn instance_ids_sorted_by_bridge_dependencies(
        &self,
    ) -> Result<Vec<String>, ConductorError> {
        let mut graph = DiGraph::<&str, &str>::new();

        // Add instance ids to the graph which returns the indices the graph is using.
        // Storing those in a map from ids to create edges from bridges below.
        let index_map: HashMap<_, _> = self
            .instances
            .iter()
            .map(|instance| (instance.id.clone(), graph.add_node(&instance.id)))
            .collect();

        // Reverse of graph indices to instance ids to create the return vector below.
        let reverse_map: HashMap<_, _> = self
            .instances
            .iter()
            .map(|instance| (index_map.get(&instance.id).unwrap(), instance.id.clone()))
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
                        "Instance configuration not found, mentioned in bridge configuration: {} -> {}",
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

        // REVERSE order because we want to get the instance with NO dependencies first
        // since that is the instance we should spawn first.
        sorted_nodes.reverse();

        // Map sorted vector of node indices back to instance ids
        Ok(sorted_nodes
            .iter()
            .map(|node_index| reverse_map.get(node_index).unwrap())
            .cloned()
            .collect())
    }

    pub fn bridge_dependencies(&self, caller_instance_id: String) -> Vec<Bridge> {
        self.bridges
            .iter()
            .filter(|bridge| bridge.caller_id == caller_instance_id)
            .cloned()
            .collect()
    }

    /// Removes the instance given by id and all mentions of it in other elements so
    /// that the config is guaranteed to be valid afterwards if it was before.
    pub fn save_remove_instance(mut self, id: &String) -> Self {
        self.instances = self
            .instances
            .into_iter()
            .filter(|instance| instance.id != *id)
            .collect();

        self.interfaces = self
            .interfaces
            .into_iter()
            .map(|mut interface| {
                interface.instances = interface
                    .instances
                    .into_iter()
                    .filter(|instance| instance.id != *id)
                    .collect();
                interface
            })
            .collect();

        self
    }

    /// This function checks if there is duplicated file storage from the instances section of a provided
    /// TOML configuration file. For efficiency purposes, we short-circuit on the first encounter of a
    /// duplicated values.
    fn check_instances_storage(&self) -> Result<(), String> {
        let storage_paths: Vec<&str> = self
            .instances
            .iter()
            .filter_map(|stg_config| match stg_config.storage {
                StorageConfig::File { ref path }
                | StorageConfig::Lmdb { ref path, .. }
                | StorageConfig::Pickle { ref path } => Some(path.as_str()),
                _ => None,
            })
            .collect();

        // Here we don't use the already implemented 'detect_dupes' function because we don't need
        // to keep track of all the duplicated values of storage instances. But instead we use the
        // return value of 'HashSet.insert()' conbined with the short-circuiting propriety of
        // 'iter().all()' so we don't iterate on all the possible value once we found a duplicated
        // storage entry.
        let mut path_set: HashSet<&str> = HashSet::new();
        let has_uniq_values = storage_paths.iter().all(|&x| path_set.insert(x));

        if !has_uniq_values {
            Err(String::from(
                "Forbidden duplicated file storage value encountered.",
            ))
        } else {
            Ok(())
        }
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

/// An instance combines a DNA with an agent.
/// Each instance has its own storage configuration.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct InstanceConfig {
    pub id: String,
    pub dna: String,
    pub agent: String,
    pub storage: StorageConfig,
}

/// This configures the Content Addressable Storage (CAS) that
/// the instance uses to store source chain and DHT shard in.
/// There are two storage implementations in cas_implementations so far:
/// * memory
/// * file
///
/// Projected are various DB adapters.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StorageConfig {
    Memory,
    File {
        path: String,
    },
    Pickle {
        path: String,
    },
    Lmdb {
        path: String,
        initial_mmap_bytes: Option<usize>,
    },
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
/// The instances (referenced by ID) that are to be made available via that interface should be listed.
/// An admin flag will enable conductor functions for programatically changing the configuration
/// (e.g. installing apps)
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct InterfaceConfig {
    pub id: String,
    pub driver: InterfaceDriver,
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub instances: Vec<InstanceReferenceConfig>,
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

/// An instance reference makes an instance available in the scope
/// of an interface.
/// Since UIs usually hard-code the name with which they reference an instance,
/// we need to decouple that name used by the UI from the internal ID of
/// the instance. That is what the optional `alias` field provides.
/// Given that there is 1-to-1 relationship between UIs and interfaces,
/// by setting an alias for available instances in the UI's interface
/// each UI can have its own unique handle for shared instances.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct InstanceReferenceConfig {
    /// ID of the instance that is made available in the interface
    pub id: String,

    /// A local name under which the instance gets mounted in the
    /// interface's scope
    pub alias: Option<String>,
}

/// A bridge enables an instance to call zome functions of another instance.
/// It is basically an internal interface.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct Bridge {
    /// ID of the instance that calls the other one.
    /// This instance depends on the callee.
    pub caller_id: String,

    /// ID of the instance that exposes traits through this bridge.
    /// This instance is used by the caller.
    pub callee_id: String,

    /// The caller's local handle of this bridge and the callee.
    /// A caller can have many bridges to other DNAs and those DNAs could
    /// by bound dynamically.
    /// Callers reference callees by this arbitrary but unique local name.
    pub handle: String,
}

/// A UI Bundle is a folder containing static assets which can be served as a UI
/// A hash can optionally be provided, which could be used to validate that the UI being installed
/// is the UI bundle that was intended to be installed.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct UiBundleConfig {
    pub id: String,
    pub root_dir: String,
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct UiInterfaceConfig {
    pub id: String,

    /// ID of the bundle to serve on this interface
    pub bundle: String,
    pub port: u16,

    /// DNA interface this UI is allowed to make calls to
    /// This is used to set the CORS headers and also to
    /// provide a extra virtual file endpoint at /_dna_config/ that allows hc-web-client
    /// or another solution to redirect holochain calls to the correct ip/port/protocol
    /// (Optional)
    #[serde(default)]
    pub dna_interface: Option<String>,

    #[serde(default = "default_reroute")]
    /// Re-route any failed HTTP Gets to /index.html
    /// This is required for SPAs using virtual routing
    /// Default = true
    pub reroute_to_root: bool,

    #[serde(default = "default_address")]
    /// Address to bind to
    /// Can be either ip4 of ip6
    /// Default = "127.0.0.1"
    pub bind_address: String,
}

fn default_reroute() -> bool {
    true
}

fn default_address() -> String {
    Ipv4Addr::LOCALHOST.to_string()
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum NetworkConfig {
    // N3h(N3hConfig),
    // Lib3h(EngineConfig),
    // Memory(EngineConfig),
    // Sim1h(Sim1hConfig),
    Sim2h(Sim2hConfig),
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct N3hConfig {
    /// List of URIs that point to other nodes to bootstrap p2p connections.
    #[serde(default)]
    pub bootstrap_nodes: Vec<String>,
    /// Global logging level output by N3H
    #[serde(default = "default_n3h_log_level")]
    pub n3h_log_level: String,
    /// Overall mode n3h operates in.
    /// Should be 'REAL'
    /// REAL is the only one and what should be used in all production cases.
    #[serde(default = "default_n3h_mode")]
    pub n3h_mode: String,
    /// Absolute path to the directory that n3h uses to store persisted data.
    #[serde(default)]
    pub n3h_persistence_path: String,
    /// URI pointing to an n3h process that is already running and not managed by this
    /// conductor.
    /// If this is set the conductor does not spawn n3h itself and ignores the path
    /// configs above. Default is None.
    #[serde(default)]
    pub n3h_ipc_uri: Option<String>,
    /// filepath to the json file holding the network settings for n3h
    #[serde(default)]
    pub networking_config_file: Option<String>,
}

// note that this behaviour is documented within
// holochain_common::env_vars module and should be updated
// if this logic changes
pub fn default_n3h_mode() -> String {
    String::from("REAL")
}

// note that this behaviour is documented within
// holochain_common::env_vars module and should be updated
// if this logic changes
pub fn default_n3h_log_level() -> String {
    String::from("i")
}

// note that this behaviour is documented within
// holochain_common::env_vars module and should be updated
// if this logic changes
pub fn default_n3h_persistence_path() -> String {
    env::temp_dir().to_string_lossy().to_string()
}

/// Use this function to load a `Config` from a string.
pub fn load_configuration<'a, T>(toml: &'a str) -> SkunkResult<T>
where
    T: Deserialize<'a>,
{
    toml::from_str::<T>(toml)
        .map_err(|e| SkunkError::Todo(format!("Error loading configuration: {}", e.to_string())))
}

pub fn serialize_configuration(config: &Config) -> SkunkResult<String> {
    // see https://github.com/alexcrichton/toml-rs/issues/142
    let config_toml = toml::Value::try_from(config).map_err(|e| {
        SkunkError::Todo(format!(
            "Could not serialize configuration: {}",
            e.to_string()
        ))
    })?;
    toml::to_string_pretty(&config_toml).map_err(|e| {
        SkunkError::Todo(format!(
            "Could not convert toml to string: {}",
            e.to_string()
        ))
    })
}

/// Configure which app instance id to treat as the DPKI application handler
/// as well as what parameters to pass it on its initialization
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct DpkiConfig {
    pub instance_id: String,
    pub init_params: String,
}

/// Configure which signals to emit, to reduce unwanted signal volume
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct SignalConfig {
    pub trace: bool,
    pub consistency: bool,
}

#[cfg(all(test, sx_refactor))]
pub mod tests {
    use super::*;
    use crate::config::{load_configuration, Config, NetworkConfig};
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
        let agents = load_configuration::<Config>(toml).unwrap().agents;
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
        let dnas = load_configuration::<Config>(toml).unwrap().dnas;
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

    [[instances]]
    id = "app spec instance"
    dna = "app spec rust"
    agent = "test agent"
        [instances.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec websocket interface"
        [interfaces.driver]
        type = "websocket"
        port = 8888
        [[interfaces.instances]]
        id = "app spec instance"

    [[interfaces]]
    id = "app spec http interface"
        [interfaces.driver]
        type = "http"
        port = 4000
        [[interfaces.instances]]
        id = "app spec instance"

    [[interfaces]]
    id = "app spec domainsocket interface"
        [interfaces.driver]
        type = "domainsocket"
        file = "/tmp/holochain.sock"
        [[interfaces.instances]]
        id = "app spec instance"

    [network]
    type = "sim2h"
    todo = "todo"

    [metric_publisher]
    type = "cloudwatchlogs"
    log_stream_name = "2019-11-22_20-53-31.sim2h_public"
    log_group_name = "holochain"

    "#;

        let config = load_configuration::<Config>(toml).unwrap();

        assert_eq!(config.check_consistency(&mut test_dna_loader()), Ok(()));
        let dnas = config.dnas;
        let dna_config = dnas.get(0).expect("expected at least 1 DNA");
        assert_eq!(dna_config.id, "app spec rust");
        assert_eq!(dna_config.file, "app_spec.dna.json");
        assert_eq!(dna_config.hash, "Qm328wyq38924y".to_string());

        let instances = config.instances;
        let instance_config = instances.get(0).unwrap();
        assert_eq!(instance_config.id, "app spec instance");
        assert_eq!(instance_config.dna, "app spec rust");
        assert_eq!(instance_config.agent, "test agent");
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

    [[instances]]
    id = "app spec instance"
    dna = "app spec rust"
    agent = "test agent"
        [instances.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec websocket interface"
        [interfaces.driver]
        type = "websocket"
        port = 8888
        [[interfaces.instances]]
        id = "app spec instance"

    [[interfaces]]
    id = "app spec http interface"
        [interfaces.driver]
        type = "http"
        port = 4000
        [[interfaces.instances]]
        id = "app spec instance"

    [[interfaces]]
    id = "app spec domainsocket interface"
        [interfaces.driver]
        type = "domainsocket"
        file = "/tmp/holochain.sock"
        [[interfaces.instances]]
        id = "app spec instance"

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

        let config = load_configuration::<Config>(toml).unwrap();

        assert_eq!(config.check_consistency(&mut test_dna_loader()), Ok(()));
        let dnas = config.dnas;
        let dna_config = dnas.get(0).expect("expected at least 1 DNA");
        assert_eq!(dna_config.id, "app spec rust");
        assert_eq!(dna_config.file, "app_spec.dna.json");
        assert_eq!(dna_config.hash, "Qm328wyq38924y".to_string());

        let instances = config.instances;
        let instance_config = instances.get(0).unwrap();
        assert_eq!(instance_config.id, "app spec instance");
        assert_eq!(instance_config.dna, "app spec rust");
        assert_eq!(instance_config.agent, "test agent");
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

    [[instances]]
    id = "app spec instance"
    dna = "app spec rust"
    agent = "test agent"
        [instances.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec websocket interface"
        [interfaces.driver]
        type = "websocket"
        port = 8888
        [[interfaces.instances]]
        id = "app spec instance"
    "#;

        let toml = format!(
            "{}{}",
            base_toml,
            r#"
    [network]
    type = "lib3h"
    "#
        );
        if let Err(e) = load_configuration::<Config>(toml.as_str()) {
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

    [[instances]]
    id = "app spec instance"
    dna = "WRONG DNA ID"
    agent = "test agent"
        [instances.storage]
        type = "file"
        path = "app_spec_storage"
    "#;

        let config: Config =
            load_configuration(toml).expect("Failed to load config from toml string");

        assert_eq!(config.check_consistency(&mut test_dna_loader()), Err("DNA configuration \"WRONG DNA ID\" not found, mentioned in instance \"app spec instance\"".to_string().into()));
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

    [[instances]]
    id = "app spec instance"
    dna = "app spec rust"
    agent = "test agent"
        [instances.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec interface"
        [interfaces.driver]
        type = "websocket"
        port = 8888
        [[interfaces.instances]]
        id = "WRONG INSTANCE ID"
    "#;

        let config = load_configuration::<Config>(toml).unwrap();

        assert_eq!(
            config.check_consistency(&mut test_dna_loader()),
            Err(
                "Instance configuration \"WRONG INSTANCE ID\" not found, mentioned in interface"
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

    [[instances]]
    id = "app spec instance"
    dna = "app spec rust"
    agent = "test agent"
    network = "{}"
        [instances.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec interface"
        [interfaces.driver]
        type = "invalid type"
        port = 8888
        [[interfaces.instances]]
        id = "app spec instance"
    "#,
            example_serialized_network_config()
        );
        if let Err(e) = load_configuration::<Config>(toml) {
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

    [[instances]]
    id = "app1"
    dna = "bridge caller"
    agent = "test agent"
        [instances.storage]
        type = "file"
        path = "app1_spec_storage"

    [[instances]]
    id = "app2"
    dna = "bridge caller"
    agent = "test agent"
        [instances.storage]
        type = "file"
        path = "app2_spec_storage"

    [[instances]]
    id = "app3"
    dna = "bridge caller"
    agent = "test agent"
        [instances.storage]
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
            load_configuration::<Config>(&toml).expect("Config should be syntactically correct");
        assert_eq!(config.check_consistency(&mut test_dna_loader()), Ok(()));

        // "->": calls
        // app1 -> app2 -> app3
        // app3 has no dependency so it can be instantiated first.
        // app2 depends on (calls) only app3, so app2 is next.
        // app1 should be last.
        assert_eq!(
            config.instance_ids_sorted_by_bridge_dependencies(),
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
            load_configuration::<Config>(&toml).expect("Config should be syntactically correct");
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
            load_configuration::<Config>(&toml).expect("Config should be syntactically correct");
        assert_eq!(
            config.check_consistency(&mut test_dna_loader()),
            Err(
                "Instance configuration \"app9000\" not found, mentioned in bridge"
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
            load_configuration::<Config>(&toml).expect("Config should be syntactically correct");
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

    [[instances]]
    id = "app spec instance"
    dna = "app spec rust"
    agent = "test agent"
        [instances.storage]
        type = "file"
        path = "app_spec_storage"

    [[interfaces]]
    id = "app spec websocket interface"
        [interfaces.driver]
        type = "websocket"
        port = 8888
        [[interfaces.instances]]
        id = "app spec instance"

    [[interfaces]]
    id = "app spec http interface"
        [interfaces.driver]
        type = "http"
        port = 4000
        [[interfaces.instances]]
        id = "app spec instance"

    [[interfaces]]
    id = "app spec domainsocket interface"
        [interfaces.driver]
        type = "domainsocket"
        file = "/tmp/holochain.sock"
        [[interfaces.instances]]
        id = "app spec instance"

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
            load_configuration::<Config>(&toml).expect("Config should be syntactically correct");
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

    [[instances]]
    id = "deepkey"
    dna = "deepkey"
    agent = "test agent"
        [instances.storage]
        type = "file"
        path = "deepkey_storage"

    [dpki]
    instance_id = "bogus instance"
    init_params = "{}"
    "#;
        let config =
            load_configuration::<Config>(&toml).expect("Config should be syntactically correct");
        assert_eq!(
            config.check_consistency(&mut test_dna_loader()),
            Err(
                "Instance configuration \"bogus instance\" not found, mentioned in dpki"
                    .to_string()
                    .into()
            )
        );
    }

    #[test]
    fn test_check_instances_storage() -> Result<(), String> {
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

        [[instances]]
        agent = "test agent 1"
        dna = "app spec rust"
        id = "app spec instance 1"

            [instances.storage]
            path = "example-config/tmp-storage-1"
            type = "file"

        [[instances]]
        agent = "test agent 2"
        dna = "app spec rust"
        id = "app spec instance 2"

            [instances.storage]
            path = "example-config/tmp-storage-2"
            type = "file"
        "#;

        let config =
            load_configuration::<Config>(&toml).expect("Config should be syntactically correct");

        assert_eq!(config.check_instances_storage(), Ok(()));
        Ok(())
    }

    #[test]
    fn test_check_instances_storage_err() -> Result<(), String> {
        // Here we have a forbidden duplicated 'instances.storage'
        let toml = r#"
        [[agents]]
        id = "test agent 1"
        keystore_file = "holo_tester.key"
        name = "Holo Tester 1"
        public_address = "HoloTester1-----------------------------------------------------------------------AAACZp4xHB"

        [[instances]]
        agent = "test agent 1"
        dna = "app spec rust"
        id = "app spec instance 1"

            [instances.storage]
            path = "forbidden-duplicated-storage-file-path"
            type = "file"

        [[instances]]
        agent = "test agent 2"
        dna = "app spec rust"
        id = "app spec instance 2"

            [instances.storage]
            path = "forbidden-duplicated-storage-file-path"
            type = "file"
        "#;

        let config =
            load_configuration::<Config>(&toml).expect("Config should be syntactically correct");

        assert_eq!(
            config.check_instances_storage(),
            Err(String::from(
                "Forbidden duplicated file storage value encountered."
            ))
        );
        Ok(())
    }
}
