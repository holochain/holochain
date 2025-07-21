//! Helpers for working with websockets and ports.

use crate::save::HcFile;
use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_conductor_api::{AdminInterfaceConfig, InterfaceDriver};
use holochain_conductor_config::config::{read_config, write_config};
use holochain_conductor_config::ports::set_admin_port;

/// Update the first admin interface to use this port.
pub fn force_admin_port(config_root_path: ConfigRootPath, port: u16) -> anyhow::Result<()> {
    let mut config =
        read_config(config_root_path.clone())?.expect("Failed to find config to force admin port");
    set_admin_port(&mut config, port);
    write_config(config_root_path, &config)?;
    Ok(())
}

/// List the admin ports for each sandbox.
pub async fn get_admin_ports(
    hc_file: &HcFile,
    paths: Vec<ConfigRootPath>,
) -> anyhow::Result<Vec<u16>> {
    let live_ports = hc_file.find_ports(&paths[..])?;
    let mut ports = Vec::new();
    for (p, port) in paths.into_iter().zip(live_ports) {
        if let Some(port) = port {
            ports.push(port);
            continue;
        }
        if let Some(config) = read_config(p)? {
            if let Some(ai) = config.admin_interfaces {
                if let Some(AdminInterfaceConfig {
                    driver: InterfaceDriver::Websocket { port, .. },
                }) = ai.first()
                {
                    ports.push(*port)
                }
            }
        }
    }
    Ok(ports)
}
