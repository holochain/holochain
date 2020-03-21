use derive_more::{From, Into};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sx_types::{cell::CellId, prelude::HashString, shims::AgentPubKey};
use url::Url;

/// The representation of persisted conductor state
pub struct ConductorManifest {
    interfaces: Vec<InterfaceManifest>,
    cells: Vec<CellManifest>,
}

#[derive(Deserialize, Serialize)]
pub struct CellManifest {
    id: CellId,
    agent: AgentManifest,
    instances: Vec<InterfaceHandle>,
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

#[derive(Deserialize, Serialize, From, Into)]
pub struct InterfaceHandle(String);

pub struct InterfaceManifest {
    driver: InterfaceDriver,
    admin: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InterfaceDriver {
    Websocket { port: u16 },
    Http { port: u16 },
    DomainSocket { file: PathBuf },
}

pub struct DnaManifest {
    location: DnaLocator,
    hash: HashString,
}

pub enum DnaLocator {
    File(PathBuf),
    Url(Url),
    Hchc,
}
