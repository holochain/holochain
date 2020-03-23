use derive_more::{From, Into};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sx_types::{cell::CellId, prelude::HashString, shims::AgentPubKey};
use url::Url;


///////////////////////////////////////////////////

/// Runtime state for a Cell, containing data which may change throughout the Conductor's execution
#[derive(Deserialize, Serialize)]
pub struct CellState {
    /// Actually the key of the database, TODO: not needed
    id: CellId,
    agent: AgentManifest,
    gateways: Vec<GatewayHandle>,
}

#[derive(Deserialize, Serialize)]
pub struct AgentManifest {
    name: String,
    public_key: AgentPubKey,
    keystore_file: PathBuf,
    /// If set to true conductor will ignore keystore_file and instead use the remote signer
    /// accessible through signing_service_uri to request signatures.
    holo_remote_key: Option<bool>,
}


pub struct GatewayManifest {
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
