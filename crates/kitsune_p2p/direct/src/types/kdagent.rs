//! kdirect kdagent type

use crate::*;
use kitsune_p2p::agent_store::*;
use kitsune_p2p::KitsuneSignature;
pub use kitsune_p2p_direct_api::{kd_agent_info::KdAgentInfoInner, KdAgentInfo};

/// Extension trait to augment the direct_api version of KdAgentInfo
pub trait KdAgentInfoExt: Sized {
    /// convert KdAgentInfo into a kitsune AgentInfoSigned
    fn to_kitsune(&self) -> AgentInfoSigned;

    /// convert a kitsune AgentInfoSigned into KdAgentInfo
    fn from_kitsune(kitsune: &AgentInfoSigned) -> KdResult<Self>;
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
        use kitsune_p2p::KitsuneBinType;

        let space = self.root().to_kitsune_space();
        let agent = self.agent().to_kitsune_agent();
        let url_list = self.url_list().iter().map(|u| u.into()).collect();
        let signed_at_ms = self.signed_at_ms() as u64;
        let expires_at_ms = self.expires_at_ms() as u64;
        let signature = Arc::new(KitsuneSignature(self.as_signature_ref().to_vec()));
        let encoded_bytes = self.as_encoded_info_ref().to_vec().into_boxed_slice();

        let center_loc = agent.get_loc().into();
        AgentInfoSigned(Arc::new(AgentInfoInner {
            space,
            agent,
            storage_arc: DhtArc {
                center_loc,
                half_length: u32::MAX, // TODO FIXME
            },
            url_list,
            signed_at_ms,
            expires_at_ms,
            signature,
            encoded_bytes,
        }))
    }

    fn from_kitsune(kitsune: &AgentInfoSigned) -> KdResult<Self> {
        let root = KdHash::from_kitsune_space(&kitsune.space);
        let agent = KdHash::from_kitsune_agent(&kitsune.agent);
        let url_list = kitsune.url_list.iter().map(|u| u.as_str().into()).collect();
        let signed_at_ms = clamp(kitsune.signed_at_ms);
        let expires_at_ms = clamp(kitsune.expires_at_ms);
        let signature = kitsune.signature.0.to_vec().into_boxed_slice().into();
        let encoded_info = kitsune.encoded_bytes.clone().into();
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
