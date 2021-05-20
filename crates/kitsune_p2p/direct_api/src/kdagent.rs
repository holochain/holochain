//! kdirect kdagent type

use crate::*;

/// Additional types associated with the KdAgentInfo struct
pub mod kd_agent_info {
    use super::*;

    /// the inner kd agent type
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct KdAgentInfoInner {
        /// the root app for this agent info
        #[serde(rename = "root")]
        pub root: KdHash,

        /// the agent pubkey
        #[serde(rename = "agent")]
        pub agent: KdHash,

        /// transport addressses this agent is reachable at
        #[serde(rename = "urlList")]
        pub url_list: Vec<String>,

        /// when this agent info record was signed
        #[serde(rename = "signedAtMs")]
        pub signed_at_ms: i64,

        /// when this agent info record will expire
        /// (note, in the raw kitsune type this is a weird offset from the signed
        ///  time, but this value is an absolute time)
        #[serde(rename = "expiresAtMs")]
        pub expires_at_ms: i64,

        /// the signature data
        #[serde(rename = "signature")]
        pub signature: kd_entry::KdEntryBinary,

        /// the raw encoded kitsune agent info
        #[serde(rename = "encodedInfo")]
        pub encoded_info: kd_entry::KdEntryBinary,
    }
}

use kd_agent_info::*;

/// a more ergonomic kdirect wrapper around the kitsune agent info type
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct KdAgentInfo(pub Arc<KdAgentInfoInner>);

impl std::fmt::Display for KdAgentInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string_pretty(&self.0).map_err(|_| std::fmt::Error)?;
        f.write_str(&s)?;
        Ok(())
    }
}

impl std::str::FromStr for KdAgentInfo {
    type Err = KdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(KdError::other)
    }
}

impl KdAgentInfo {
    /// Reconstruct this KdAgentINfo from a `to_string()` str.
    // this *does* implement the trait clippy...
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> KdResult<Self> {
        std::str::FromStr::from_str(s)
    }

    /// get the raw signature bytes of the kitsune AgentInfoSigned
    pub fn as_signature_ref(&self) -> &[u8] {
        &self.0.signature
    }

    /// get the raw encoded bytes of the kitsune AgentInfoSigned
    pub fn as_encoded_info_ref(&self) -> &[u8] {
        &self.0.encoded_info
    }

    /// get the root app hash
    pub fn root(&self) -> &KdHash {
        &self.0.root
    }

    /// get the agent hash
    pub fn agent(&self) -> &KdHash {
        &self.0.agent
    }

    /// get the url_list
    pub fn url_list(&self) -> &[String] {
        &self.0.url_list
    }

    /// get the signed_at_ms
    pub fn signed_at_ms(&self) -> i64 {
        self.0.signed_at_ms
    }

    /// get the expires_at_ms
    pub fn expires_at_ms(&self) -> i64 {
        self.0.expires_at_ms
    }
}
