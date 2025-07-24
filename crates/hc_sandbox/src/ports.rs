//! Helpers for working with websockets and ports.

use holochain_conductor_api::conductor::paths::ConfigRootPath;
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
