//! Data structures to be stored in the agent/peer database.

use crate::types::KitsuneAgent;
use crate::types::KitsuneBinType;
use crate::types::KitsuneP2pError;
use crate::types::KitsuneSignature;
use crate::types::KitsuneSpace;
use url2::Url2;

/// A list of Urls.
pub type Urls = Vec<Url2>;

/// A space/agent pair that defines the key for AgentInfo across all spaces.
#[derive(Debug)]
pub struct AgentInfoSignedKey {
    space: KitsuneSpace,
    agent: KitsuneAgent,
}

impl AgentInfoSignedKey {
    /// Wraps get_bytes for the space.
    pub fn space_bytes(&self) -> &[u8] {
        &self.space.get_bytes()
    }

    /// Wrapgs get_bytes for the agent.
    pub fn agent_bytes(&self) -> &[u8] {
        &self.agent.get_bytes()
    }
}

impl From<(KitsuneSpace, KitsuneAgent)> for AgentInfoSignedKey {
    fn from((space, agent): (KitsuneSpace, KitsuneAgent)) -> Self {
        Self { space, agent }
    }
}

/// Value in the peer database that tracks an Agent's representation as signed by that agent.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, derive_more::AsRef)]
pub struct AgentInfoSigned {
    // Raw bytes of agent info signature as kitsune signature.
    signature: KitsuneSignature,
    // The agent info.
    agent_info: AgentInfo,
}

impl From<&AgentInfoSigned> for AgentInfoSignedKey {
    fn from(agent_info_signed: &AgentInfoSigned) -> Self {
        Self {
            space: agent_info_signed
                .as_agent_info_ref()
                .as_space_ref()
                .to_owned(),
            agent: agent_info_signed
                .as_agent_info_ref()
                .as_agent_ref()
                .to_owned(),
        }
    }
}

impl AgentInfoSigned {
    /// Build a new AgentInfoSigned struct given a valid signature of the AgentInfo.
    // @todo fail this if the signature does not verify against the agent info.
    // It should not be possible to express a signed agent info type  with no valid signature.
    pub fn try_new(
        signature: KitsuneSignature,
        agent_info: AgentInfo,
    ) -> Result<Self, KitsuneP2pError> {
        Ok(Self {
            signature,
            agent_info,
        })
    }

    /// Thin wrapper around AsRef for KitsuneSignature.
    pub fn as_signature_ref(&self) -> &KitsuneSignature {
        self.as_ref()
    }

    /// Thin wrapper around AsRef for AgentInfo
    pub fn as_agent_info_ref(&self) -> &AgentInfo {
        self.as_ref()
    }
}

/// Value that an agent signs to represent themselves on the network.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, derive_more::AsRef)]
pub struct AgentInfo {
    // The space this agent info is relevant to.
    space: KitsuneSpace,
    // The pub key of the agent id this info is relevant to.
    agent: KitsuneAgent,
    // List of urls the agent can be reached at, in the agent's own preference order.
    urls: Urls,
    // The unix ms timestamp that the agent info was signed at, according to the agent's own clock.
    signed_at_ms: u64,
}

impl AgentInfo {
    /// Constructor.
    pub fn new(space: KitsuneSpace, agent: KitsuneAgent, urls: Urls, signed_at_ms: u64) -> Self {
        Self {
            space,
            agent,
            urls,
            signed_at_ms,
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
}
