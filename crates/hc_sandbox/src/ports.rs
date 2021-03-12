//! Helpers for working with websockets and ports.
use std::path::PathBuf;
use std::sync::Arc;

use holochain_conductor_api::{
    config::conductor::ConductorConfig, AdminInterfaceConfig, InterfaceDriver,
};
use holochain_websocket::{self as ws, WebsocketConfig, WebsocketReceiver, WebsocketSender};
use url2::prelude::*;
use ws::WebsocketResult;

use crate::config::read_config;
use crate::config::write_config;

/// Update the first admin interface to use this port.
pub fn force_admin_port(path: PathBuf, port: u16) -> anyhow::Result<()> {
    let mut config = read_config(path.clone())?.expect("Failed to find config to force admin port");
    set_admin_port(&mut config, port);
    write_config(path, &config);
    Ok(())
}

/// List the admin ports for each sandbox.
pub async fn get_admin_ports(paths: Vec<PathBuf>) -> anyhow::Result<Vec<u16>> {
    let live_ports = crate::save::find_ports(std::env::current_dir()?, &paths[..])?;
    let mut ports = Vec::new();
    for (p, port) in paths.into_iter().zip(live_ports) {
        if let Some(port) = port {
            ports.push(port);
            continue;
        }
        if let Some(config) = read_config(p)? {
            if let Some(ai) = config.admin_interfaces {
                if let Some(AdminInterfaceConfig {
                    driver: InterfaceDriver::Websocket { port },
                }) = ai.get(0)
                {
                    ports.push(*port)
                }
            }
        }
    }
    Ok(ports)
}

pub(crate) async fn get_admin_api(port: u16) -> WebsocketResult<WebsocketSender> {
    tracing::debug!(port);
    websocket_client_by_port(port).await.map(|p| p.0)
}

async fn websocket_client_by_port(
    port: u16,
) -> WebsocketResult<(WebsocketSender, WebsocketReceiver)> {
    Ok(ws::connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?)
}

pub(crate) fn random_admin_port(config: &mut ConductorConfig) {
    match config.admin_interfaces.as_mut().and_then(|i| i.first_mut()) {
        Some(AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port },
        }) => {
            if *port != 0 {
                *port = 0;
            }
        }
        None => {
            let port = 0;
            config.admin_interfaces = Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port },
            }]);
        }
    }
}

pub(crate) fn set_admin_port(config: &mut ConductorConfig, port: u16) {
    let p = port;
    let port = AdminInterfaceConfig {
        driver: InterfaceDriver::Websocket { port },
    };
    match config
        .admin_interfaces
        .as_mut()
        .and_then(|ai| ai.get_mut(0))
    {
        Some(admin_interface) => {
            *admin_interface = port;
        }
        None => config.admin_interfaces = Some(vec![port]),
    }
    msg!("Admin port set to: {}", p);
}
