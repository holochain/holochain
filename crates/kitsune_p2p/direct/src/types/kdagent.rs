//! kdirect kdagent type

use crate::*;
use kitsune_p2p::agent_store::*;
use kitsune_p2p::KitsuneSignature;
use std::convert::TryFrom;

pub use kitsune_p2p_direct_api::{kd_agent_info::KdAgentInfoInner, KdAgentInfo};

/// Extension trait to augment the direct_api version of KdAgentInfo
pub trait KdAgentInfoExt: Sized {
    /// convert KdAgentInfo into a kitsune AgentInfoSigned
    fn to_kitsune(&self) -> AgentInfoSigned;

    /// convert a kitsune AgentInfoSigned into KdAgentInfo
    fn from_kitsune(kitsune: &AgentInfoSigned) -> KitsuneResult<Self>;
}

fn clamp(u: u64) -> i64 {
    if u > i64::MAX as u64 {
        i64::MAX
    } else {
        u as i64
    }
}

impl KdAgentInfoExt for KdAgentInfo {
    fn to_kitsune(&self) -> AgentInfoSigned {
        let agent = (*self.agent().to_kitsune_agent()).clone();
        let signature = KitsuneSignature(self.as_signature_ref().to_vec());
        let agent_info = self.as_encoded_info_ref().to_vec();
        AgentInfoSigned::new_unchecked(agent, signature, agent_info)
    }

    fn from_kitsune(kitsune: &AgentInfoSigned) -> KitsuneResult<Self> {
        let i = AgentInfo::try_from(kitsune).map_err(KitsuneError::other)?;
        let root = KdHash::from_kitsune_space(i.as_space_ref());
        let agent = KdHash::from_kitsune_agent(i.as_agent_ref());
        let url_list = i.as_urls_ref().iter().map(|u| u.clone().into()).collect();
        let signed_at_ms = clamp(i.signed_at_ms());
        let expires_at_ms = signed_at_ms + clamp(i.expires_after_ms());
        let signature = kitsune
            .as_signature_ref()
            .0
            .to_vec()
            .into_boxed_slice()
            .into();
        let encoded_info = kitsune
            .as_agent_info_ref()
            .to_vec()
            .into_boxed_slice()
            .into();
        Ok(Self(Arc::new(KdAgentInfoInner {
            root,
            agent,
            url_list,
            signed_at_ms,
            expires_at_ms,
            signature,
            encoded_info,
        })))
    }
}
