use serde::Deserialize;
use serde::Serialize;

/// Information neeeded to spawn an Admin interface
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq)]
pub struct AdminInterfaceConfig {
    /// By what means will the interface be exposed?
    /// Current only option is a local websocket running on a configurable port.
    pub driver: InterfaceDriver,
    // /// How long will this interface be accessible between authentications?
    // /// TODO: implement once we have authentication
    // _session_duration_seconds: Option<u32>,
}

/// Configuration for interfaces, specifying the means by which an interface
/// should be opened.
///
/// NB: This struct is used in both [ConductorConfig] and [ConductorState], so
/// any change to the serialization strategy is a **breaking change**.
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InterfaceDriver {
    /// An interface implemented via Websockets
    Websocket {
        /// The port on which to establish the WebsocketListener
        port: u16,
    },
}

impl InterfaceDriver {
    /// Get the port for this driver.
    pub fn port(&self) -> u16 {
        match self {
            InterfaceDriver::Websocket { port } => *port,
        }
    }
}
