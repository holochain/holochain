//! Helpers for working with ports.

use holochain_conductor_api::{
    config::conductor::ConductorConfig, AdminInterfaceConfig, InterfaceDriver,
};
use holochain_types::websocket::AllowedOrigins;

pub fn random_admin_port(config: &mut ConductorConfig) {
    match config.admin_interfaces.as_mut().and_then(|i| i.first_mut()) {
        Some(AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket { port, .. },
        }) => {
            if *port != 0 {
                *port = 0;
            }
        }
        None => {
            let port = 0;
            config.admin_interfaces = Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket {
                    port,
                    allowed_origins: AllowedOrigins::Any,
                },
            }]);
        }
    }
}
