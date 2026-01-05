use holochain_types::websocket::AllowedOrigins;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

/// Information neeeded to spawn an admin interface
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminInterfaceConfig {
    /// By what means the interface will be exposed.
    ///
    /// Currently, the only option is a local websocket running on a configurable port.
    pub driver: InterfaceDriver,
}

/// Configuration for interfaces, specifying the means by which an interface
/// should be opened.
///
/// **NB**: This struct is used in both [`ConductorConfig`]
/// and [`ConductorState`],
/// so any change to the serialization strategy is a breaking change.
///
/// [`ConductorConfig`]: crate::conductor::ConductorConfig
/// [`ConductorState`]: https://docs.rs/holochain/latest/holochain/conductor/state/struct.ConductorState.html
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum InterfaceDriver {
    /// An interface implemented via websockets
    Websocket {
        /// The port on which to establish the WebsocketListener
        port: u16,

        /// An optional address to bind the interface to.
        ///
        /// This is dangerous to set to anything other than `localhost`. If no value is set then
        /// the interface will bind to `localhost`.
        ///
        /// Holochain has minimal security protections in place for websocket connections. The app
        /// websockets are protected by the admin websocket, but if you expose the admin websocket
        /// to the network, then anyone who can connect to it can control your conductor.
        danger_bind_addr: Option<String>,

        /// Allowed origins for this interface.
        ///
        /// This should be one of:
        /// - A comma separated list of origins - `http://localhost:3000,http://localhost:3001`,
        /// - A single origin - `http://localhost:3000`,
        /// - Any origin - `*`
        ///
        /// Connections from any origin which is not permitted by this config will be rejected.
        allowed_origins: AllowedOrigins,
    },
}

impl InterfaceDriver {
    /// Get the port for this driver.
    pub fn port(&self) -> u16 {
        match self {
            InterfaceDriver::Websocket { port, .. } => *port,
        }
    }

    /// The address override to bind the interface to, if any.
    pub fn danger_bind_addr(&self) -> Option<&String> {
        match self {
            InterfaceDriver::Websocket {
                danger_bind_addr, ..
            } => danger_bind_addr.as_ref(),
        }
    }

    /// Get the allowed origins for this driver.
    pub fn allowed_origins(&self) -> &AllowedOrigins {
        match self {
            InterfaceDriver::Websocket {
                allowed_origins, ..
            } => allowed_origins,
        }
    }
}
