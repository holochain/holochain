//! kdirect kdagent type

use crate::*;
use kitsune_p2p::agent_store::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::convert::TryFrom;
use types::kdhash::KdHash;

/// the inner kd agent type
pub struct KdAgentInfoInner {
    /// the root app for this agent info
    pub root: KdHash,
    /// the agent pubkey
    pub agent: KdHash,
    /// transport addressses this agent is reachable at
    pub urls: Vec<TxUrl>,
    /// when this agent info record was signed
    pub signed_at_ms: u64,
    /// the raw kitsune agent info type
    pub raw: AgentInfoSigned,
}

impl std::fmt::Debug for KdAgentInfoInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KdAgentInfoInner")
            .field("root", &self.root)
            .field("agent", &self.agent)
            .field("urls", &self.urls)
            .field("signed_at_ms", &self.signed_at_ms)
            .finish()
    }
}

/// a more ergonomic kdirect wrapper around the kitsune agent info type
#[derive(Clone, Debug)]
pub struct KdAgentInfo(pub Arc<KdAgentInfoInner>);

impl std::ops::Deref for KdAgentInfo {
    type Target = KdAgentInfoInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<KdAgentInfo> for AgentInfoSigned {
    fn from(f: KdAgentInfo) -> AgentInfoSigned {
        f.0.raw.clone()
    }
}

impl KdAgentInfo {
    /// wrap a kitsune agent info type
    pub fn new(f: AgentInfoSigned) -> KitsuneResult<Self> {
        let i = AgentInfo::try_from(&f).map_err(KitsuneError::other)?;
        assert_eq!(f.as_agent_ref(), i.as_agent_ref());
        let root = i.as_space_ref().into();
        let agent = i.as_agent_ref().into();
        let signed_at_ms = i.signed_at_ms();
        let urls = i.as_urls_ref().iter().map(|u| u.clone().into()).collect();
        Ok(Self(Arc::new(KdAgentInfoInner {
            root,
            agent,
            urls,
            signed_at_ms,
            raw: f,
        })))
    }
}
