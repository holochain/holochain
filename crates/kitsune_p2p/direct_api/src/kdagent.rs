//! kdirect kdagent type

use crate::*;
use kitsune_p2p_dht_arc::{DhtArcRange, DhtLocation};

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

        /// The storage arc currently being published by this agent.
        pub storage_arc: DhtArcRange,

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

impl PartialEq for KdAgentInfo {
    fn eq(&self, oth: &Self) -> bool {
        self.0.encoded_info.eq(&*oth.0.encoded_info)
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

    /// get the storage arc
    pub fn storage_arc(&self) -> &DhtArcRange {
        &self.0.storage_arc
    }

    /// Get the distance from a basis to this agent's storage arc.
    /// Will be zero if this agent covers this basis loc.
    pub fn basis_distance_to_storage(&self, basis: DhtLocation) -> u32 {
        match self.storage_arc().to_bounds_grouped() {
            None => u32::MAX,
            Some((s, e)) => {
                let basis = basis.as_u32();
                let s = s.as_u32();
                let e = e.as_u32();
                if s <= e {
                    if basis >= s && basis <= e {
                        0
                    } else if basis < s {
                        std::cmp::min(s - basis, (u32::MAX - e) + basis)
                    } else {
                        std::cmp::min(basis - e, (u32::MAX - basis) + s)
                    }
                } else if basis <= e || basis >= s {
                    0
                } else {
                    std::cmp::min(basis - e, s - basis)
                }
            }
        }
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
