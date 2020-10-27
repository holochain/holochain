//! Legacy Conductor Configuration
//!
//! This module provides a subset of the Configuration struct present in holochain-redux,
//! for backward compatibility purposes, mainly for interoperability with Tryorama.
//! The holochain::conductor::compat module contains a function for converting this
//! struct into a `(Config, ConductorState)` pair.

use serde::*;
use std::{collections::HashMap, path::PathBuf};
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

    /// Configures Jaeger tracing. Optional.
    // #[serde(default)]
    // pub tracing: Option<Box<dyn Any>>,

    // /// Configuration options for the network module. Optional.
    // #[serde(default)]
    // pub network: Option<NetworkConfig>,

    /// where to persist the config file and DNAs. Optional.
    // #[serde(default = "default_persistence_dir")]
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
    // #[serde(default)]
    // pub metric_publisher: Option<Box<dyn Any>>,
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
    FromConfig { passphrase: String },
}

impl Default for PassphraseServiceConfig {
    fn default() -> PassphraseServiceConfig {
        PassphraseServiceConfig::Cmd
    }
}

/// This is a config helper structure used to interface with the holochain logging subcrate.
/// Custom rules/filter can be applied to logging, in fact they are used by default in Holochain to
/// filter the logs from its dependencies.
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

// Don't care about this value; just get deserialization right
type LogRules = Vec<HashMap<String, String>>;

impl Default for LoggerConfiguration {
    fn default() -> LoggerConfiguration {
        LoggerConfiguration {
            logger_level: "debug".into(),
            rules: Default::default(),
            state_dump: false,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TracingConfiguration {
    None,
    Jaeger(JaegerTracingConfiguration),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct JaegerTracingConfiguration {
    pub service_name: String,
    pub socket_address: Option<String>,
}

impl Default for TracingConfiguration {
    fn default() -> Self {
        TracingConfiguration::None
    }
}

/// An agent has a name/ID and is optionally defined by a private key that resides in a file
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub public_address: String,
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

/// An instance combines a DNA with an agent.
/// Each instance has its own storage configuration.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct InstanceConfig {
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
pub struct InterfaceConfig {
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

    // #[serde(default = "default_reroute")]
    /// Re-route any failed HTTP Gets to /index.html
    /// This is required for SPAs using virtual routing
    /// Default = true
    pub reroute_to_root: bool,

    // #[serde(default = "default_address")]
    /// Address to bind to
    /// Can be either ip4 of ip6
    /// Default = "127.0.0.1"
    pub bind_address: String,
}

// #[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
// #[serde(rename_all = "snake_case")]
// #[serde(tag = "type")]
// #[allow(clippy::large_enum_variant)]
// pub enum NetworkConfig {
//     Lib3h(EngineConfig),
//     Memory(EngineConfig),
//     Sim2h(Sim2hConfig),
// }

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

impl Config {
    /// Returns the agent configuration with the given ID if present
    pub fn agent_by_id(&self, id: &str) -> Option<&AgentConfig> {
        self.agents.iter().find(|ac| ac.id == id)
    }

    /// Returns the DNA configuration with the given ID if present
    pub fn dna_by_id(&self, id: &str) -> Option<&DnaConfig> {
        self.dnas.iter().find(|dc| dc.id == id)
    }

    /// Returns the instance configuration with the given ID if present
    pub fn instance_by_id(&self, id: &str) -> Option<&InstanceConfig> {
        self.instances.iter().find(|ic| ic.id == id)
    }

    /// Returns the interface configuration with the given ID if present
    pub fn interface_by_id(&self, id: &str) -> Option<&InterfaceConfig> {
        self.interfaces.iter().find(|ic| ic.id == id)
    }
}
