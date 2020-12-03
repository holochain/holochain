//! Data structures to be stored in the agent/peer database.

use crate::types::KitsuneAgent;
use crate::types::KitsuneP2pError;
use crate::types::KitsuneSignature;
use crate::types::KitsuneSpace;
use url2::Url2;

/// A list of Urls.
pub type Urls = Vec<Url2>;

/// Value in the peer database that tracks an Agent's representation as signed by that agent.
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Debug,
    PartialEq,
    derive_more::AsRef,
    std::cmp::Ord,
    std::cmp::Eq,
    std::cmp::PartialOrd,
)]
pub struct AgentInfoSigned {
    // Agent public key that needs to be the same as the agent in the signed agent_info.
    agent: KitsuneAgent,
    // Raw bytes of agent info signature as kitsune signature.
    signature: KitsuneSignature,
    // The agent info as encoded MessagePack data as the exact bytes signed by the signature above.
    #[serde(with = "serde_bytes")]
    agent_info: Vec<u8>,
}

impl AgentInfoSigned {
    /// Build a new AgentInfoSigned struct given a valid signature of the AgentInfo.
    // @todo fail this if the signature does not verify against the agent info.
    // It should not be possible to express a signed agent info type  with no valid signature.
    pub fn try_new(
        agent: KitsuneAgent,
        signature: KitsuneSignature,
        agent_info: Vec<u8>,
    ) -> Result<Self, KitsuneP2pError> {
        Ok(Self {
            agent,
            signature,
            agent_info,
        })
    }

    /// Thin wrapper around AsRef for KitsuneSignature.
    pub fn as_signature_ref(&self) -> &KitsuneSignature {
        self.as_ref()
    }

    /// Thin wrapper around AsRef for KitsuneAgent.
    pub fn as_agent_ref(&self) -> &KitsuneAgent {
        self.as_ref()
    }

    /// Thin wrapper around Into for KitsuneAgent.
    pub fn into_agent(self) -> KitsuneAgent {
        self.into()
    }

    /// Thin wrapper around AsRef for AgentInfo
    pub fn as_agent_info_ref(&self) -> &[u8] {
        self.agent_info.as_ref()
    }
}

/// Value that an agent signs to represent themselves on the network.
#[derive(
    serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, derive_more::AsRef, Hash, Eq,
)]
pub struct AgentInfo {
    // The space this agent info is relevant to.
    space: KitsuneSpace,
    // The pub key of the agent id this info is relevant to.
    agent: KitsuneAgent,
    // List of urls the agent can be reached at, in the agent's own preference order.
    urls: Urls,
    // The unix ms timestamp that the agent info was signed at, according to the agent's own clock.
    #[as_ref(ignore)]
    signed_at_ms: u64,
    // The expiry ttl for the agent info relative to the signing time.
    #[as_ref(ignore)]
    expires_after_ms: u64,
}

impl std::convert::TryFrom<&AgentInfoSigned> for AgentInfo {
    type Error = KitsuneP2pError;
    fn try_from(agent_info_signed: &AgentInfoSigned) -> Result<Self, Self::Error> {
        Ok(kitsune_p2p_types::codec::rmp_decode(
            &mut &*agent_info_signed.agent_info,
        )?)
    }
}

impl AgentInfo {
    /// Constructor.
    pub fn new(
        space: KitsuneSpace,
        agent: KitsuneAgent,
        urls: Urls,
        signed_at_ms: u64,
        expires_after_ms: u64,
    ) -> Self {
        Self {
            space,
            agent,
            urls,
            signed_at_ms,
            expires_after_ms,
        }
    }
}

impl AsRef<[Url2]> for AgentInfo {
    fn as_ref(&self) -> &[Url2] {
        &self.urls
    }
}

impl AgentInfo {
    /// Thin AsRef wrapper for space.
    pub fn as_space_ref(&self) -> &KitsuneSpace {
        self.as_ref()
    }

    /// Thin AsRef wrapper for agent.
    pub fn as_agent_ref(&self) -> &KitsuneAgent {
        self.as_ref()
    }

    /// Thin AsRef wrapper for urls.
    pub fn as_urls_ref(&self) -> &[Url2] {
        self.as_ref()
    }

    /// Accessor for signed_at_ms.
    pub fn signed_at_ms(&self) -> u64 {
        self.signed_at_ms
    }

    /// Accessor for expires_after_ms.
    pub fn expires_after_ms(&self) -> u64 {
        self.expires_after_ms
    }
}

impl From<AgentInfoSigned> for KitsuneAgent {
    fn from(ai: AgentInfoSigned) -> Self {
        ai.agent
    }
}

impl From<AgentInfo> for KitsuneAgent {
    fn from(ai: AgentInfo) -> Self {
        ai.agent
    }
}
