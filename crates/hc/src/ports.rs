//! Helpers for working with websockets and ports.
use std::path::PathBuf;
use std::sync::Arc;

use holochain_conductor_api::{
    config::conductor::ConductorConfig, AdminInterfaceConfig, InterfaceDriver,
};
use holochain_websocket::{websocket_connect, WebsocketConfig, WebsocketReceiver, WebsocketSender};
use portpicker::is_free;
use portpicker::pick_unused_port;
use url2::prelude::*;

use crate::config::read_config;
use crate::config::write_config;

/// Update the first admin interface to use this port.
pub fn force_admin_port(path: PathBuf, port: u16) -> anyhow::Result<()> {
    let mut config = read_config(path.clone())?.expect("Failed to find config to force admin port");
    set_admin_port(&mut config, port);
    write_config(path, &config);
    Ok(())
}

/// Add a secondary admin port to the conductor config.
pub fn add_secondary_admin_port(path: PathBuf, port: Option<u16>) -> anyhow::Result<()> {
    let mut config = read_config(path.clone())?.expect("Failed to find config to force admin port");
    set_secondary_admin_port(&mut config, port);
    write_config(path, &config);
    Ok(())
}

/// List the secondary ports for each setup.
pub async fn get_secondary_admin_ports(paths: Vec<PathBuf>) -> anyhow::Result<Vec<u16>> {
    let mut ports = Vec::new();
    for p in paths {
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

pub(crate) async fn get_admin_api(port: u16) -> std::io::Result<WebsocketSender> {
    tracing::debug!(port);
    websocket_client_by_port(port).await.map(|p| p.0)
}

async fn websocket_client_by_port(
    port: u16,
) -> std::io::Result<(WebsocketSender, WebsocketReceiver)> {
    Ok(websocket_connect(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?)
}

pub(crate) fn random_admin_port_if_busy(config: &mut ConductorConfig) -> u16 {
    match config.admin_interfaces.as_mut().and_then(|i| i.first_mut()) {
        Some(AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port },
        }) => {
            if !is_free(*port) {
                *port = pick_unused_port().expect("No ports free");
            }
            *port
        }
        None => {
            let port = pick_unused_port().expect("No ports free");
            config.admin_interfaces = Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port },
            }]);
            port
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

pub(crate) fn set_secondary_admin_port(config: &mut ConductorConfig, port: Option<u16>) {
    let port = port.unwrap_or_else(|| pick_unused_port().expect("No ports free"));
    let p = port;
    let port = AdminInterfaceConfig {
        driver: InterfaceDriver::Websocket { port },
    };
    match config
        .admin_interfaces
        .as_mut()
        // .and_then(|ai| ai)
    {
        Some(admin_interface) if admin_interface.len() == 1 => {
            admin_interface.push(port);
        }
        Some(admin_interface) if admin_interface.len() >= 2 => {
            admin_interface[1] = port;
        }
        Some(_) | None => {
            random_admin_port_if_busy(config);
            config.admin_interfaces = Some(vec![port])
        }
    }
    msg!("Secondary admin port Admin port set to: {}", p);
}
