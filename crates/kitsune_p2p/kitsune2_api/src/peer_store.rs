//! Peer-store related types.

use crate::*;

/// Represents the ability to store and query agents.
pub trait PeerStore: 'static + Send + Sync + std::fmt::Debug {
    /// Clear (delete) the entire store.
    /// Leave it ready to accept insertion of new entries.
    fn clear(&self) -> BoxFuture<'_, Result<()>>;

    /// Insert agents into the store.
    fn insert(&self, agent_list: Vec<agent::DynAgentInfo>) -> BoxFuture<'_, Result<()>>;

    /// Get an agent from the store.
    fn get(&self, agent: DynId) -> BoxFuture<'_, Result<Option<agent::DynAgentInfo>>>;

    /// Get all agents from the store.
    fn get_all(&self) -> BoxFuture<'_, Result<Vec<agent::DynAgentInfo>>>;

    /// Get multiple agents from the store.
    fn get_many(&self, agent_list: Vec<DynId>) -> BoxFuture<'_, Result<Vec<agent::DynAgentInfo>>>;

    /// Query the peer store by time and arq bounds.
    fn query_by_time_and_arq(
        &self,
        since: Timestamp,
        until: Timestamp,
        arq: arq::DynArq,
    ) -> BoxFuture<'_, Result<Vec<agent::DynAgentInfo>>>;

    /// Query the peer store by location nearness.
    fn query_by_location(
        &self,
        loc: u32,
        limit: usize,
    ) -> BoxFuture<'_, Result<Vec<agent::DynAgentInfo>>>;
}

/// Trait-object [PeerStore].
pub type DynPeerStore = Arc<dyn PeerStore>;

/// A factory for constructing PeerStore instances.
pub trait PeerStoreFactory: 'static + Send + Sync + std::fmt::Debug {
    /// Construct a peer store instance.
    fn create(&self, builder: Arc<builder::Builder>) -> BoxFuture<'static, Result<DynPeerStore>>;
}

/// Trait-object [PeerStoreFactory].
pub type DynPeerStoreFactory = Arc<dyn PeerStoreFactory>;
