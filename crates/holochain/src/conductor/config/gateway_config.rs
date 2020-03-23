use derive_more::{From, Into};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// ID used to refer to a running Gateway.
/// This ID is referenced in hApp bundles
#[derive(Deserialize, Serialize, From, Into)]
pub struct GatewayId(String);

pub struct GatewayConfig {
    id: GatewayId,
    driver: GatewayDriver,
    admin: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum GatewayDriver {
    Websocket { port: u16 },
    Http { port: u16 },
    DomainSocket { file: PathBuf },
}
