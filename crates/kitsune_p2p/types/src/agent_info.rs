//! Data structures to be stored in the agent/peer database.

use crate::bin_types::*;
use crate::dht_arc::DhtArc;
use crate::tx_utils::TxUrl;
use crate::*;
use agent_info_helper::*;
use dht::Arq;

/// A list of Urls.
pub type UrlList = Vec<TxUrl>;

/// An agent paired with its storage arc in interval form
pub type AgentArc = (Arc<KitsuneAgent>, DhtArc);

/// agent_info helper types
pub mod agent_info_helper {
    use dht::arq::ArqSize;

    use super::*;

    #[allow(missing_docs)]
    #[derive(Debug, serde::Serialize, serde::Deserialize, derive_more::From, derive_more::Into)]
    pub struct AgentMetaInfoEncode {
        pub arq_size: ArqSize,
    }

    impl From<Arq> for AgentMetaInfoEncode {
        fn from(arq: Arq) -> Self {
            ArqSize::from(arq).into()
        }
    }

    #[allow(missing_docs)]
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct AgentInfoEncode {
        pub space: Arc<KitsuneSpace>,
        pub agent: Arc<KitsuneAgent>,
        pub urls: UrlList,
        pub signed_at_ms: u64,

        /// WARNING-this is a weird offset from the signed_at_ms time!!!!
        pub expires_after_ms: u64,

        #[serde(with = "serde_bytes")]
        pub meta_info: Box<[u8]>,
    }

    #[allow(missing_docs)]
    #[derive(Debug, serde::Deserialize)]
    pub struct AgentInfoSignedEncode {
        pub agent: Arc<KitsuneAgent>,
        pub signature: Arc<KitsuneSignature>,
        #[serde(with = "serde_bytes")]
        pub agent_info: Box<[u8]>,
    }

    #[allow(missing_docs)]
    #[derive(Debug, serde::Serialize)]
    pub struct AgentInfoSignedEncodeRef<'lt> {
        pub agent: &'lt Arc<KitsuneAgent>,
        pub signature: &'lt Arc<KitsuneSignature>,
        #[serde(with = "serde_bytes")]
        pub agent_info: &'lt [u8],
    }
}

/// The inner constructable AgentInfo struct
pub struct AgentInfoInner {
    /// The space this agent info is relevant to.
    pub space: Arc<KitsuneSpace>,

    /// The pub key of the agent id this info is relevant to.
    pub agent: Arc<KitsuneAgent>,

    /// The storage arc currently being published by this agent.
    pub storage_arq: Arq,

    /// List of urls the agent can be reached at, in the agent's own preference order.
    pub url_list: UrlList,

    /// The absolute unix ms timestamp that the agent info was signed at,
    /// according to the agent's own clock.
    pub signed_at_ms: u64,

    /// The absolute unix ms timestamp this info will expire at,
    /// according to the agent's own clock.
    /// Note--the encoded bootstrap version of this struct uses a weird
    /// offset from the signed time... but this value here is the more
    /// intuitive absolute value.
    pub expires_at_ms: u64,

    /// Raw bytes of agent info signature as kitsune signature.
    pub signature: Arc<KitsuneSignature>,

    /// the raw encoded bytes sent to bootstrap server to use for sig verify.
    pub encoded_bytes: Box<[u8]>,
}

impl AgentInfoInner {
    /// If this agent is considered active.
    pub fn is_active(&self) -> bool {
        !self.url_list.is_empty()
    }
}

impl std::fmt::Debug for AgentInfoInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentInfoSigned")
            .field("space", &self.space)
            .field("agent", &self.agent)
            .field("storage_arq", &self.storage_arq)
            .field("url_list", &self.url_list)
            .field("signed_at_ms", &self.signed_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

impl PartialEq for AgentInfoInner {
    fn eq(&self, oth: &Self) -> bool {
        self.encoded_bytes.eq(&oth.encoded_bytes)
    }
}

impl Eq for AgentInfoInner {}

impl std::hash::Hash for AgentInfoInner {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.encoded_bytes.hash(state);
    }
}

/// Value in the peer database that tracks an Agent's representation as signed by that agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentInfoSigned(pub Arc<AgentInfoInner>);

impl std::ops::Deref for AgentInfoSigned {
    type Target = AgentInfoInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl serde::Serialize for AgentInfoSigned {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let encode = AgentInfoSignedEncodeRef {
            agent: &self.agent,
            signature: &self.signature,
            agent_info: &self.encoded_bytes,
        };
        encode.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for AgentInfoSigned {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let AgentInfoSignedEncode {
            agent,
            signature,
            agent_info,
        } = AgentInfoSignedEncode::deserialize(deserializer)?;

        let mut bytes: &[u8] = &agent_info;
        let info: AgentInfoEncode =
            crate::codec::rmp_decode(&mut bytes).map_err(serde::de::Error::custom)?;
        let mut bytes: &[u8] = &info.meta_info;
        let meta: AgentMetaInfoEncode =
            crate::codec::rmp_decode(&mut bytes).map_err(serde::de::Error::custom)?;

        if agent != info.agent {
            return Err(serde::de::Error::custom("agent mismatch"));
        }

        let storage_arq = meta.arq_size.to_arq(agent.get_loc());

        let AgentInfoEncode {
            space,
            agent,
            urls,
            signed_at_ms,
            expires_after_ms,
            ..
        } = info;

        let inner = AgentInfoInner {
            space,
            agent,
            storage_arq,
            url_list: urls,
            signed_at_ms,
            expires_at_ms: signed_at_ms + expires_after_ms,
            signature,
            encoded_bytes: agent_info,
        };

        Ok(AgentInfoSigned(Arc::new(inner)))
    }
}

impl AgentInfoSigned {
    /// Construct and sign a new AgentInfoSigned instance.
    pub async fn sign<'a, R, F>(
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        meta: impl Into<AgentMetaInfoEncode>,
        url_list: UrlList,
        signed_at_ms: u64,
        expires_at_ms: u64,
        f: F,
    ) -> KitsuneResult<Self>
    where
        R: std::future::Future<Output = KitsuneResult<Arc<KitsuneSignature>>>,
        F: FnOnce(&[u8]) -> R,
    {
        let meta = meta.into();
        let storage_arq = meta.arq_size.to_arq(agent.get_loc());

        let mut buf = Vec::new();
        crate::codec::rmp_encode(&mut buf, meta).map_err(KitsuneError::other)?;
        let meta = buf.into_boxed_slice();

        let info = AgentInfoEncode {
            space: space.clone(),
            agent: agent.clone(),
            urls: url_list.clone(),
            signed_at_ms,
            expires_after_ms: expires_at_ms - signed_at_ms,
            meta_info: meta,
        };
        let mut buf = Vec::new();
        crate::codec::rmp_encode(&mut buf, info).map_err(KitsuneError::other)?;
        let encoded_bytes = buf.into_boxed_slice();

        let signature = f(&encoded_bytes).await?;

        let inner = AgentInfoInner {
            space,
            agent,
            storage_arq,
            url_list,
            signed_at_ms,
            expires_at_ms,
            signature,
            encoded_bytes,
        };

        Ok(Self(Arc::new(inner)))
    }

    /// decode from msgpack
    pub fn decode(b: &[u8]) -> KitsuneResult<Self> {
        let mut bytes: &[u8] = b;
        crate::codec::rmp_decode(&mut bytes).map_err(KitsuneError::other)
    }

    /// encode as msgpack
    pub fn encode(&self) -> KitsuneResult<Box<[u8]>> {
        let mut buf = Vec::new();
        crate::codec::rmp_encode(&mut buf, self).map_err(KitsuneError::other)?;
        Ok(buf.into_boxed_slice())
    }

    /// Accessor
    pub fn agent(&self) -> Arc<KitsuneAgent> {
        self.agent.clone()
    }

    /// Convert arq to arc
    pub fn storage_arc(&self) -> DhtArc {
        self.storage_arq.to_dht_arc_std()
    }
}

#[cfg(test)]
mod tests {
    use dht::arq::ArqSize;

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn agent_info() {
        let space = Arc::new(KitsuneSpace(vec![0x01; 36]));
        let agent = Arc::new(KitsuneAgent(vec![0x02; 36]));

        let info = AgentInfoSigned::sign(
            space.clone(),
            agent.clone(),
            ArqSize::empty(),
            vec![],
            42,
            69,
            |_| async move { Ok(Arc::new(vec![0x03; 64].into())) },
        )
        .await
        .unwrap();

        assert_eq!(info.space, space);
        assert_eq!(info.agent, agent);

        let mut enc = Vec::new();
        crate::codec::rmp_encode(&mut enc, &info).unwrap();
        let mut bytes: &[u8] = &enc;
        let info2: AgentInfoSigned = crate::codec::rmp_decode(&mut bytes).unwrap();
        assert_eq!(info, info2);
    }
}
