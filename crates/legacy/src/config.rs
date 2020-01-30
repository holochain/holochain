use crate::{conductor::base::DnaLoader, logger::LogRules};
/// Conductor Configuration
/// This module provides structs that represent the different aspects of how
/// a conductor can be configured.
/// This mainly means *listing the instances* the conductor tries to instantiate and run,
/// plus the resources needed by these instances:
/// * agents
/// * DNAs, i.e. the custom app code that makes up the core of a Holochain instance
/// * interfaces, which in this context means ways for user interfaces, either GUIs or local
///   scripts or other local apps, to call DNAs' zome functions and call admin functions of
///   the conductor
/// * bridges, which are

use boolinator::*;
use sx_types::prelude::*;
use lib3h::engine::EngineConfig;

use holochain_metrics::MetricPublisherConfig;
use holochain_net::{sim1h_worker::Sim1hConfig, sim2h_worker::Sim2hConfig};
use serde::{Serialize, Deserialize};
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
use toml;
/// Main conductor configuration struct
/// This is the root of the configuration tree / aggregates
/// all other configuration aspects.
///
/// References between structs (instance configs pointing to
/// the agent and DNA to be instantiated) are implemented
/// via string IDs.
#[derive(Deserialize, Serialize, Clone, Default, Debug)]
pub struct Configuration {
    /// List of Agents, this mainly means identities and their keys. Required.
    pub agents: Vec<AgentConfiguration>,
    /// List of DNAs, for each a path to the DNA file. Optional.
    #[serde(default)]
    pub dnas: Vec<DnaConfiguration>,
    /// List of instances, includes references to an agent and a DNA. Optional.
    #[serde(default)]
    pub instances: Vec<InstanceConfiguration>,
    /// List of interfaces any UI can use to access zome functions. Optional.
    #[serde(default)]
    pub interfaces: Vec<InterfaceConfiguration>,

    /// List of bridges between instances. Optional.
    #[serde(default)]
    pub bridges: Vec<Bridge>,

    /// !DEPRECATION WARNING! - Hosting a static UI via the conductor will not be supported in future releases
    /// List of ui bundles (static web dirs) to host on a static interface. Optional.
    #[serde(default)]
    pub ui_bundles: Vec<UiBundleConfiguration>,
    /// List of ui interfaces, includes references to ui bundles and dna interfaces it can call. Optional.
    #[serde(default)]
    pub ui_interfaces: Vec<UiInterfaceConfiguration>,

    /// Configures how logging should behave. Optional.
    #[serde(default)]
    pub logger: LoggerConfiguration,
    /// Configuration options for the network module. Optional.
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
    pub dpki: Option<DpkiConfiguration>,

    /// Which signals to emit
    #[serde(default)]
    pub signals: SignalConfig,

    /// Configure how the conductor should prompt the user for the passphrase to lock/unlock keystores.
    /// The conductor is independent of the specialized implementation of the trait
    /// PassphraseService. It just needs something to provide a passphrase when needed.
    /// This config setting selects one of the available services (i.e. CLI prompt, IPC, mock)
    #[serde(default)]
    pub passphrase_service: PassphraseServiceConfig,

    #[serde(default)]
    pub metric_publisher: Option<MetricPublisherConfig>,
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

/// This is a config helper structure used to interface with the holochain logging subcrate.
/// Custom rules/filter can be applied to logging, in fact they are used by default in Holochain to
/// filter the logs from its dependencies.
///
/// ```rust
/// extern crate holochain_conductor_lib;
/// use holochain_conductor_lib::{logger,config};
/// let mut rules = logger::LogRules::new();
/// // Filtering out all the logs from our dependencies
/// rules
///     .add_rule(".*", true, None)
///     .expect("Invalid logging rule.");
/// // And logging back all Holochain logs
/// rules
///     .add_rule("^holochain", false, None)
///     .expect("Invalid logging rule.");
///
/// let lc = config::LoggerConfiguration {
///     logger_level: "debug".to_string(),
///     rules: rules,
///     state_dump: true,
///     };
/// ```
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct LoggerConfiguration {
    #[serde(rename = "type")]
    pub logger_level: String,
    #[serde(default)]
    pub rules: LogRules,
    //    pub file: Option<String>,
    #[serde(default)]
    pub state_dump: bool,
}

impl Default for LoggerConfiguration {
    fn default() -> LoggerConfiguration {
        LoggerConfiguration {
            logger_level: "debug".into(),
            rules: Default::default(),
            state_dump: false,
        }
    }
}

/// An agent has a name/ID and is optionally defined by a private key that resides in a file
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AgentConfiguration {
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

impl From<AgentConfiguration> for AgentId {
    fn from(config: AgentConfiguration) -> Self {
        AgentId::try_from(JsonString::from_json(&config.id)).expect("bad agent json")
    }
}

/// A DNA is represented by a DNA file.
/// A hash can optionally be provided, which could be used to validate that the DNA being installed
/// is the DNA that was intended to be installed.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct DnaConfiguration {
    pub id: String,
    pub file: String,
    pub hash: String,
    #[serde(default)]
    pub uuid: Option<String>,
}

impl TryFrom<DnaConfiguration> for Dna {
    type Error = HolochainError;
    fn try_from(dna_config: DnaConfiguration) -> Result<Self, Self::Error> {
        let mut f = File::open(dna_config.file)?;
        let mut contents = String::new();
        f.read_to_string(&mut contents)?;
        Dna::try_from(JsonString::from_json(&contents)).map_err(|err| err.into())
    }
}

/// An instance combines a DNA with an agent.
/// Each instance has its own storage configuration.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct InstanceConfiguration {
    pub id: String,
    pub dna: String,
    pub agent: String,
    pub storage: StorageConfiguration,
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
pub enum StorageConfiguration {
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
pub struct InterfaceConfiguration {
    pub id: String,
    pub driver: InterfaceDriver,
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub instances: Vec<InstanceReferenceConfiguration>,
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
pub struct InstanceReferenceConfiguration {
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
pub struct UiBundleConfiguration {
    pub id: String,
    pub root_dir: String,
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct UiInterfaceConfiguration {
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


/// Configure which app instance id to treat as the DPKI application handler
/// as well as what parameters to pass it on its initialization
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct DpkiConfiguration {
    pub instance_id: String,
    pub init_params: String,
}

/// Configure which signals to emit, to reduce unwanted signal volume
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct SignalConfig {
    pub trace: bool,
    pub consistency: bool,
}
