// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use holochain::sweettest::{SweetConductor, SweetConductorConfig, SweetLocalRendezvous};
use holochain_conductor_api::{AdminInterfaceConfig, InterfaceDriver};
use holochain_types::websocket::AllowedOrigins;
use kitsune2_api::{DynLocalAgent, SpaceId};
use kitsune2_core::Ed25519LocalAgent;
use kitsune2_test_utils::agent::AgentBuilder;
use std::sync::Arc;

pub fn make_agent(space: &SpaceId) -> String {
    AgentBuilder {
        space_id: Some(space.clone()),
        ..Default::default()
    }
    .build(Arc::new(Ed25519LocalAgent::default()) as DynLocalAgent)
    .encode()
    .unwrap()
}

/// Picks a port that is free at this instant.
///
/// Restart tests need the admin port to stay put, which the standard sweettest
/// config does not do because it binds port 0.
pub fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

/// Builds a conductor whose admin interface keeps the same port across a
/// shutdown and startup cycle.
pub async fn conductor_with_fixed_admin_port() -> (SweetConductor, u16) {
    let port = free_port();
    (conductor_on_admin_port(port).await, port)
}

/// Builds a conductor whose admin interface listens on `port`.
pub async fn conductor_on_admin_port(port: u16) -> SweetConductor {
    let mut config = SweetConductorConfig::rendezvous(true);
    config.admin_interfaces = Some(vec![AdminInterfaceConfig {
        driver: InterfaceDriver::Websocket {
            port,
            danger_bind_addr: None,
            allowed_origins: AllowedOrigins::Any,
        },
    }]);
    SweetConductor::from_config_rendezvous(config, SweetLocalRendezvous::new().await).await
}
