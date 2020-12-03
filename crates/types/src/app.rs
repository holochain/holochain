//! Collection of cells to form a holochain application
use crate::{cell::CellId, dna::JsonProperties};
use derive_more::Into;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::SerializedBytes;
use std::path::PathBuf;

/// Placeholder used to identify installed apps
pub type InstalledAppId = String;

/// A friendly (nick)name used by UIs to refer to the Cells which make up the app
pub type CellNick = String;

/// A collection of [DnaHash]es paired with an [AgentPubKey] and an app id
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppPayload {
    /// Placeholder to find the installed app
    pub installed_app_id: InstalledAppId,
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
    /// The CellNick which will be assigned to this Dna when installed
    pub nick: CellNick,
    /// Properties to override when installing this Dna
    pub properties: Option<JsonProperties>,
    /// App-specific proof-of-membrane-membership, if required by this app
    pub membrane_proof: Option<MembraneProof>,
}

impl InstallAppDnaPayload {
    /// Create a payload with no JsonProperties or MembraneProof. Good for tests.
    pub fn path_only(path: PathBuf, nick: CellNick) -> Self {
        Self {
            path,
            nick,
            properties: None,
            membrane_proof: None,
        }
    }
}

/// App-specific payload for proving membership in the membrane of the app
pub type MembraneProof = SerializedBytes;

/// Data about an installed Cell
#[derive(Clone, Debug, Into, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledCell(CellId, CellNick);

impl InstalledCell {
    /// Constructor
    pub fn new(cell_id: CellId, cell_handle: CellNick) -> Self {
        Self(cell_id, cell_handle)
    }

    /// Get the CellId
    pub fn into_id(self) -> CellId {
        self.0
    }

    /// Get the CellNick
    pub fn into_nick(self) -> CellNick {
        self.1
    }

    /// Get the inner data as a tuple
    pub fn into_inner(self) -> (CellId, CellNick) {
        (self.0, self.1)
    }

    /// Get the CellId
    pub fn as_id(&self) -> &CellId {
        &self.0
    }

    /// Get the CellNick
    pub fn as_nick(&self) -> &CellNick {
        &self.1
    }
}

/// A collection of [InstalledCell]s paired with an app id
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledApp {
    /// Placeholder to find the app
    pub installed_app_id: InstalledAppId,
    /// Cell data for this app
    pub cell_data: Vec<InstalledCell>,
}
