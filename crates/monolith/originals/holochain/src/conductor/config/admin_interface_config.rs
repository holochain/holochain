#![deny(missing_docs)]

use monolith::holochain::conductor::interface::InterfaceDriver;
use serde::Deserialize;
use serde::Serialize;

/// Information neeeded to spawn an Admin interface
#[derive(Clone, Deserialize, Serialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct AdminInterfaceConfig {
    /// By what means will the interface be exposed?
    /// Current only option is a local websocket running on a configurable port.
    pub driver: InterfaceDriver,
    // /// How long will this interface be accessible between authentications?
    // /// TODO: implement once we have authentication
    // _session_duration_seconds: Option<u32>,
}
