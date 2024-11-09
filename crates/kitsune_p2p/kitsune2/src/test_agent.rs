use crate::*;

/// A test identifier.
pub struct TestId {
    /// bytes.
    pub bytes: Bytes,

    /// loc.
    pub loc: u32,
}

impl std::fmt::Debug for TestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TestId").field(&self.loc).finish()
    }
}

impl std::fmt::Display for TestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Id for TestId {
    fn bytes(&self) -> Bytes {
        self.bytes.clone()
    }

    fn loc(&self) -> u32 {
        self.loc
    }
}

impl Default for TestId {
    fn default() -> Self {
        static UNIQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let bytes = UNIQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let bytes = bytes.to_le_bytes();
        let bytes = Bytes::copy_from_slice(&bytes);
        use rand::Rng;
        Self {
            bytes,
            loc: rand::thread_rng().gen(),
        }
    }
}

impl TestId {
    /// Convert to the dyn type.
    pub fn into_dyn(self) -> DynId {
        let out: DynId = Arc::new(self);
        out
    }
}

/// A test agent.
#[derive(Debug)]
pub struct TestAgentInfo {
    /// The agent identifier.
    pub id: DynId,

    /// Is this agent active?
    pub is_active: bool,

    /// When was this agent created?
    pub created_at: Timestamp,

    /// When will this agent expire?
    pub expires_at: Timestamp,

    /// What is this agent storing?
    pub storage_arq: arq::DynArq,
}

impl agent::AgentInfo for TestAgentInfo {
    fn id(&self) -> &DynId {
        &self.id
    }

    fn is_active(&self) -> bool {
        self.is_active
    }

    fn created_at(&self) -> Timestamp {
        self.created_at
    }

    fn expires_at(&self) -> Timestamp {
        self.expires_at
    }

    fn storage_arq(&self) -> &arq::DynArq {
        &self.storage_arq
    }
}

impl Default for TestAgentInfo {
    fn default() -> Self {
        Self {
            id: TestId::default().into_dyn(),
            is_active: true,
            created_at: Timestamp::now(),
            expires_at: Timestamp::now() + std::time::Duration::from_secs(60 * 20),
            storage_arq: arq::ArqFull::create(),
        }
    }
}

impl TestAgentInfo {
    /// Convert to the dyn type.
    pub fn into_dyn(self) -> agent::DynAgentInfo {
        let out: agent::DynAgentInfo = Arc::new(self);
        out
    }
}
