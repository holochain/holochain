use crate::*;

/// An in-memory PeerStore factory.
#[derive(Debug)]
pub struct MemPeerStoreFactory {
    _p: (),
}

impl MemPeerStoreFactory {
    /// Create a new MemPeerStoreFactory.
    pub fn create() -> peer_store::DynPeerStoreFactory {
        let out: peer_store::DynPeerStoreFactory = Arc::new(Self { _p: () });
        out
    }
}

impl peer_store::PeerStoreFactory for MemPeerStoreFactory {
    fn create(
        &self,
        _builder: Arc<builder::Builder>,
    ) -> BoxFuture<'static, Result<peer_store::DynPeerStore>> {
        let out: peer_store::DynPeerStore = Arc::new(MemPeerStore::new());
        Box::pin(async move { Ok(out) })
    }
}

struct MemPeerStore(Mutex<Inner>);

impl std::fmt::Debug for MemPeerStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemPeerStore").finish()
    }
}

impl MemPeerStore {
    pub fn new() -> Self {
        Self(Mutex::new(Inner::new()))
    }
}

impl peer_store::PeerStore for MemPeerStore {
    fn insert(&self, agent: agent::DynAgentInfo) -> BoxFuture<'_, Result<()>> {
        self.0.lock().unwrap().insert(agent);
        Box::pin(async move { Ok(()) })
    }

    fn get(&self, agent: DynId) -> BoxFuture<'_, Result<Option<agent::DynAgentInfo>>> {
        let r = self.0.lock().unwrap().get(agent);
        Box::pin(async move { Ok(r) })
    }

    fn query_by_time_and_arq(
        &self,
        since: Timestamp,
        until: Timestamp,
        arq: arq::DynArq,
    ) -> BoxFuture<'_, Result<Vec<agent::DynAgentInfo>>> {
        let r = self
            .0
            .lock()
            .unwrap()
            .query_by_time_and_arq(since, until, arq);
        Box::pin(async move { Ok(r) })
    }

    fn query_by_location(
        &self,
        loc: u32,
        limit: usize,
    ) -> BoxFuture<'_, Result<Vec<agent::DynAgentInfo>>> {
        let r = self.0.lock().unwrap().query_by_location(loc, limit);
        Box::pin(async move { Ok(r) })
    }
}

struct Inner {
    store: HashMap<Bytes, agent::DynAgentInfo>,
    no_prune_until: std::time::Instant,
}

impl Inner {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
            no_prune_until: std::time::Instant::now() + std::time::Duration::from_secs(10),
        }
    }

    fn check_prune(&mut self) {
        // use an instant here even though we have to create a Timestamp::now()
        // below, because it's faster to query than SystemTime if we're aborting
        let inst_now = std::time::Instant::now();
        if self.no_prune_until > inst_now {
            return;
        }

        let now = Timestamp::now();

        self.store.retain(|_, v| v.expires_at() > now);

        // we only care about not looping on the order of tight cpu cycles
        // even a couple seconds gets us away from this.
        self.no_prune_until = inst_now + std::time::Duration::from_secs(10)
    }

    pub fn insert(&mut self, agent_info: agent::DynAgentInfo) {
        self.check_prune();

        // Don't insert expired infos.
        if agent_info.expires_at() < Timestamp::now() {
            return;
        }

        if let Some(a) = self.store.get(&agent_info.id().bytes()) {
            // If we already have a newer (or equal) one, abort.
            if a.created_at() >= agent_info.created_at() {
                return;
            }
        }

        self.store.insert(agent_info.id().bytes(), agent_info);
    }

    pub fn get(&mut self, agent: DynId) -> Option<agent::DynAgentInfo> {
        self.check_prune();

        self.store.get(&agent.bytes()).cloned()
    }

    pub fn query_by_time_and_arq(
        &mut self,
        since: Timestamp,
        until: Timestamp,
        arq: arq::DynArq,
    ) -> Vec<agent::DynAgentInfo> {
        self.check_prune();

        self.store
            .values()
            .filter_map(|info| {
                if !info.is_active() {
                    return None;
                }

                if info.created_at() < since {
                    return None;
                }

                if info.created_at() > until {
                    return None;
                }

                if !arq.overlap(&info.storage_arq()) {
                    return None;
                }

                Some(info.clone())
            })
            .collect()
    }

    pub fn query_by_location(&mut self, basis: u32, limit: usize) -> Vec<agent::DynAgentInfo> {
        self.check_prune();

        let mut out: Vec<(u32, &agent::DynAgentInfo)> = self
            .store
            .values()
            .filter_map(|v| {
                if v.is_active() {
                    Some((v.storage_arq().dist(basis), v))
                } else {
                    None
                }
            })
            .collect();

        if out.len() > 1 {
            out.sort_by(|a, b| a.0.cmp(&b.0));
        }

        out.into_iter()
            .filter(|(dist, _)| *dist != u32::MAX) // Filter out Zero arcs
            .take(limit)
            .map(|(_, v)| v.clone())
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn empty_query_by_loc() {
        let store = builder::Builder::create_default()
            .create_peer_store()
            .await
            .unwrap();
        assert_eq!(0, store.query_by_location(0, 1).await.unwrap().len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn happy_store_and_get() {
        let store = builder::Builder::create_default()
            .create_peer_store()
            .await
            .unwrap();
        let agent = TestAgentInfo::default().into_dyn();
        store.insert(agent.clone()).await.unwrap();
        let got = store.get(agent.id().clone()).await.unwrap().unwrap();
        assert_eq!(agent.id().to_string(), got.id().to_string());
    }
}
