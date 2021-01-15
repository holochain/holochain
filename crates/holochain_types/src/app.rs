//! Collection of cells to form a holochain application
use crate::{dna::JsonProperties, dna_bundle::DnaBundleManifest};
use derive_more::Into;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::SerializedBytes;
use holochain_zome_types::cell::CellId;
use std::path::PathBuf;

/// The unique identifier for an installed app in this conductor
pub type InstalledAppId = String;

/// A friendly (nick)name used by UIs to refer to the Cells which make up the app
pub type CellNick = String;

/// A collection of [DnaHash]es paired with an [AgentPubKey] and an app id
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum InstallAppPayload {
    /// Used to specify DNAs on-the-fly
    Explicit(InstallAppPayloadNormalized),
    /// Used to specify DNAs per a bundle file
    Bundle {
        /// The agent to use when creating Cells for this App
        agent_key: AgentPubKey,

        /// The DNA bundle manifest for this app
        // TODO: this will probably actually be a file path or raw file data
        //       that gets deserialized
        dna_bundle: DnaBundleManifest,
    },
}

impl InstallAppPayload {
    /// Collapse the two variants down to a common normalized structure
    pub fn normalize(self) -> InstallAppPayloadNormalized {
        match self {
            InstallAppPayload::Explicit(payload) => payload,
            InstallAppPayload::Bundle {
                agent_key,
                dna_bundle,
            } => todo!(),
        }
    }
}

/// A normalized structure common to both variants of InstallAppPayload
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppPayloadNormalized {
    /// The unique identifier for an installed app in this conductor
    pub installed_app_id: InstalledAppId,

    /// The agent to use when creating Cells for this App
    pub agent_key: AgentPubKey,

    /// The Dna paths in this app
    pub dnas: Vec<InstallAppDnaPayload>,
}

impl From<InstallAppPayloadNormalized> for InstallAppPayload {
    fn from(p: InstallAppPayloadNormalized) -> Self {
        InstallAppPayload::Explicit(p)
    }
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
