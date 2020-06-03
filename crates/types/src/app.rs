//! Collection of cells to form a holochain application
use crate::{cell::CellId, dna::JsonProperties};
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::SerializedBytes;
use std::{collections::HashMap, path::PathBuf};

/// Placeholder used to identify apps
pub type AppId = String;

/// A friendly handled used by UIs to refer to the Cells which make up the app
pub type CellHandle = String;

/// A collection of [DnaHash]es paired with an [AgentPubKey] and an app id
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppPayload {
    /// Placeholder to find the app
    pub app_id: AppId,
    /// The agent that installed this app
    pub agent_key: AgentPubKey,
    /// The Dna paths in this app
    pub dnas: HashMap<CellHandle, InstallAppDnaPayload>,
}

/// Information needed to specify a Dna as part of an App
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppDnaPayload {
    /// The path of the DnaFile
    pub path: PathBuf,
    /// Properties to override when installing this Dna
    pub properties: Option<JsonProperties>,
    /// App-specific proof-of-membrane-membership, if required by this app
    pub membrane_proof: Option<MembraneProof>,
}

impl InstallAppDnaPayload {
    /// Create a payload with no JsonProperties or MembraneProof. Good for tests.
    pub fn path_only(path: PathBuf) -> Self {
        Self {
            path,
            properties: None,
            membrane_proof: None,
        }
    }
}

/// App-specific payload for proving membership in the membrane of the app
pub type MembraneProof = SerializedBytes;

/// App storage
pub type InstalledApps = HashMap<AppId, Vec<CellId>>;

#[derive(Clone, Debug)]
/// A collection of [CellIds]s paired with an app id
pub struct InstalledApp {
    /// Placeholder to find the app
    pub app_id: AppId,
    /// Cells in this app
    pub cell_ids: Vec<CellId>,
}
