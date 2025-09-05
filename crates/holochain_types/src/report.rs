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

impl ReportEntryFetchedOps {
    /// Verify the signatures.
    pub fn verify(&self, verifier: &kitsune2_api::DynVerifier) -> bool {
        use base64::prelude::*;

        let mut to_verify = Vec::new();
        to_verify.extend_from_slice(self.timestamp.as_bytes());
        to_verify.extend_from_slice(self.space.as_bytes());
        to_verify.extend_from_slice(self.op_count.as_bytes());
        to_verify.extend_from_slice(self.total_bytes.as_bytes());
        for a in self.agent_pubkeys.iter() {
            to_verify.extend_from_slice(a.as_bytes());
        }

        const STUB_ID: bytes::Bytes = bytes::Bytes::from_static(b"");
        let stub_ts = Timestamp::now();
        for (agent, sig) in self.agent_pubkeys.iter().zip(self.signatures.iter()) {
            let agent = match BASE64_URL_SAFE_NO_PAD.decode(agent) {
                Ok(agent) => agent,
                Err(_) => return false,
            };
            let sig = match BASE64_URL_SAFE_NO_PAD.decode(sig) {
                Ok(sig) => sig,
                Err(_) => return false,
            };
            // for ed25519 verification, we only need
            // the agent part of the agent info, so the rest can be stubbed
            let agent = kitsune2_api::AgentInfo {
                agent: bytes::Bytes::copy_from_slice(&agent).into(),
                space: STUB_ID.into(),
                created_at: stub_ts.into(),
                expires_at: stub_ts.into(),
                is_tombstone: false,
                url: None,
                storage_arc: kitsune2_api::DhtArc::default(),
            };

            if !verifier.verify(&agent, &to_verify, &sig) {
                return false;
            }
        }

        true
    }
}
