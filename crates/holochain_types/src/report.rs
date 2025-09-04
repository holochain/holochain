//! Types associated with holochain reporting.

use kitsune2_api::Timestamp;

/// Holochain reporting entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "k", rename_all="camelCase")]
pub enum ReportEntry {
    /// Indicates that the holochain process has started.
    Start(ReportEntryStart),

    /// Reports a receipt indicating that a peer received fetched ops from us.
    FetchedOps(ReportEntryFetchedOps),
}

impl ReportEntry {
    /// Create a new "start" entry.
    pub fn start() -> Self {
        Self::Start(ReportEntryStart {
            timestamp: Timestamp::now().as_micros().to_string(),
        })
    }
}

/// Indicates that the holochain process has started.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ReportEntryStart {
    /// Timestamp microseconds since unix epoch.
    #[serde(rename = "t")]
    pub timestamp: String,
}

/// Reports a receipt indicating that a peer received fetched ops from us.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ReportEntryFetchedOps {
    /// Timestamp microseconds since unix epoch.
    #[serde(rename = "t")]
    pub timestamp: String,

    /// Space id as a base64url string.
    #[serde(rename = "d")]
    pub space: String,

    /// Op count as a string.
    #[serde(rename = "c")]
    pub op_count: String,

    /// Total byte length of all ops as a string.
    #[serde(rename = "b")]
    pub total_bytes: String,

    /// List of base64url agent pubkeys from the node that received the op data.
    #[serde(rename = "a")]
    pub agent_pubkeys: Vec<String>,

    /// Signatures generated and verifyable by the listed agent pubkeys.
    ///
    /// Concat the utf8 bytes of timestamp, space, op_count, total_bytes and
    /// agent_pubkeys in order to generate/validate the signatures.
    #[serde(rename = "s")]
    pub signatures: Vec<String>,
}
