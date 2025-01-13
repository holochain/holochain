//! Helpers for working with ports.

use holochain_conductor_api::{
    config::conductor::ConductorConfig, AdminInterfaceConfig, InterfaceDriver,
};
use holochain_types::websocket::AllowedOrigins;

use crate::msg;

pub fn set_admin_port(config: &mut ConductorConfig, port: u16) {
    let p = port;
    let port = AdminInterfaceConfig {
        driver: InterfaceDriver::Websocket {
            port,
            allowed_origins: AllowedOrigins::Any,
        },
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
