//! Types associated with holochain reporting.

use kitsune2_api::Timestamp;

/// Holochain reporting entry.
///
/// When encoded as json, the tag property is "k" (kind) for brevity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "k", rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct ReportEntryStart {
    /// Timestamp microseconds since unix epoch.
    ///
    /// When encoded as json, the property is "t" (timestamp) for brevity.
    #[serde(rename = "t")]
    pub timestamp: String,
}

/// Reports a receipt indicating that a peer received fetched ops from us.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEntryFetchedOps {
    /// Timestamp microseconds since unix epoch.
    ///
    /// When encoded as json, the property is "t" (timestamp) for brevity.
    #[serde(rename = "t")]
    pub timestamp: String,

    /// Space id as a base64url string.
    ///
    /// When encoded as json, the property is "d" (dna) for brevity.
    #[serde(rename = "d")]
    pub space: String,

    /// Op count as a string.
    ///
    /// When encoded as json, the property is "c" (count) for brevity.
    #[serde(rename = "c")]
    pub op_count: String,

    /// Total byte length of all ops as a string.
    ///
    /// When encoded as json, the property is "b" (bytes) for brevity.
    #[serde(rename = "b")]
    pub total_bytes: String,

    /// List of base64url agent pubkeys from the node that received the op data.
    ///
    /// When encoded as json, the property is "a" (agent) for brevity.
    #[serde(rename = "a")]
    pub agent_pubkeys: Vec<String>,

    /// Signatures generated and verifyable by the listed agent pubkeys.
    ///
    /// Concat the utf8 bytes of timestamp, space, op_count, total_bytes and
    /// agent_pubkeys in order to generate/validate the signatures.
    ///
    /// When encoded as json, the property is "s" (signatures) for brevity.
    #[serde(rename = "s")]
    pub signatures: Vec<String>,
}

impl ReportEntryFetchedOps {
    /// Generate the canonical encoded byte array of this entry
    /// for signing and verification.
    pub fn encode_for_verification(&self) -> Vec<u8> {
        let mut len =
            self.timestamp.len() + self.space.len() + self.op_count.len() + self.total_bytes.len();
        for a in self.agent_pubkeys.iter() {
            len += a.len();
        }
        let mut out = Vec::with_capacity(len);
        out.extend_from_slice(self.timestamp.as_bytes());
        out.extend_from_slice(self.space.as_bytes());
        out.extend_from_slice(self.op_count.as_bytes());
        out.extend_from_slice(self.total_bytes.as_bytes());
        for a in self.agent_pubkeys.iter() {
            out.extend_from_slice(a.as_bytes());
        }
        out
    }

    /// Verify the signatures.
    pub async fn verify(&self) -> bool {
        use base64::prelude::*;

        let to_verify = self.encode_for_verification();

        for (agent, sig) in self.agent_pubkeys.iter().zip(self.signatures.iter()) {
            let agent = match BASE64_URL_SAFE_NO_PAD.decode(agent) {
                Ok(agent) => agent,
                Err(_) => return false,
            };
            let sig = match BASE64_URL_SAFE_NO_PAD.decode(sig) {
                Ok(sig) => sig,
                Err(_) => return false,
            };
            let sig: [u8; 64] = sig.try_into().expect("array convesion failed");

            let mut pk = Vec::with_capacity(36);
            pk.extend_from_slice(&agent);
            pk.extend_from_slice(&[0, 0, 0, 0]);
            let pk = crate::prelude::AgentPubKey::from_raw_36(pk);
            use holochain_keystore::AgentPubKeyExt;
            match pk
                .verify_signature_raw(&sig.into(), to_verify.clone().into())
                .await
            {
                Ok(true) => (),
                _ => return false,
            }
        }

        true
    }
}
