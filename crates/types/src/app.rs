//! Collection of cells to form a holochain application
use crate::{cell::CellId, dna::JsonProperties};
use holo_hash::{AgentPubKey, DnaHash};
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
    pub dnas: Vec<(PathBuf, Option<JsonProperties>)>,
    /// A map of [DnaHash] to proofs
    pub proofs: HashMap<DnaHash, SerializedBytes>,
}

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
