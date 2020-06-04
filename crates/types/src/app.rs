//! Collection of cells to form a holochain application
use crate::{cell::CellId, dna::JsonProperties};
use derive_more::Into;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::SerializedBytes;
use std::path::PathBuf;

/// Placeholder used to identify apps
pub type AppId = String;

/// A friendly handle used by UIs to refer to the Cells which make up the app
pub type CellHandle = String;

/// A collection of [DnaHash]es paired with an [AgentPubKey] and an app id
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppPayload {
    /// Placeholder to find the app
    pub app_id: AppId,
    /// The agent that installed this app
    pub agent_key: AgentPubKey,
    /// The Dna paths in this app
    pub dnas: Vec<InstallAppDnaPayload>,
}

/// Information needed to specify a Dna as part of an App
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppDnaPayload {
    /// The path of the DnaFile
    pub path: PathBuf,
    /// The CellHandle which will be assigned to this Dna when installed
    pub handle: CellHandle,
    /// Properties to override when installing this Dna
    pub properties: Option<JsonProperties>,
    /// App-specific proof-of-membrane-membership, if required by this app
    pub membrane_proof: Option<MembraneProof>,
}

impl InstallAppDnaPayload {
    /// Create a payload with no JsonProperties or MembraneProof. Good for tests.
    pub fn path_only(path: PathBuf, handle: String) -> Self {
        Self {
            path,
            handle,
            properties: None,
            membrane_proof: None,
        }
    }
}

/// App-specific payload for proving membership in the membrane of the app
pub type MembraneProof = SerializedBytes;

/// Data about an installed Cell
#[derive(Clone, Debug, Into, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledCell(CellId, CellHandle);

impl InstalledCell {
    /// Constructor
    pub fn new(cell_id: CellId, cell_handle: CellHandle) -> Self {
        Self(cell_id, cell_handle)
    }

    /// Get the CellId
    pub fn into_id(self) -> CellId {
        self.0
    }

    /// Get the CellHandle
    pub fn into_handle(self) -> CellHandle {
        self.1
    }
    /// Get the CellId
    pub fn as_id(&self) -> &CellId {
        &self.0
    }

    /// Get the CellHandle
    pub fn as_handle(&self) -> &CellHandle {
        &self.1
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
/// A collection of [CellIds]s paired with an app id
pub struct InstalledApp {
    /// Placeholder to find the app
    pub app_id: AppId,
    /// Cell data for this app
    pub cell_data: Vec<InstalledCell>,
}
