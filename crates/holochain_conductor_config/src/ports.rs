//! Helpers for working with ports.

use crate::msg;
use holochain_conductor_api::{
    config::conductor::ConductorConfig, AdminInterfaceConfig, InterfaceDriver,
};
use holochain_types::websocket::AllowedOrigins;

pub fn set_admin_port(config: &mut ConductorConfig, port: u16) {
    match config
        .admin_interfaces
        .as_mut()
        .and_then(|ai| ai.get_mut(0))
    {
        Some(admin_interface) => {
            *admin_interface = AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port,
                    allowed_origins: admin_interface.driver.allowed_origins().to_owned(),
                },
            };
        }
        None => {
            config.admin_interfaces = Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port,
                    allowed_origins: AllowedOrigins::Any,
                },
            }])
        }
    }
    msg!("Admin port set to: {}", port);
}
