//! kdirect persist type

use crate::*;
use futures::future::BoxFuture;
use kitsune_p2p::dht_arc::DhtArcSet;
use kitsune_p2p::event::MetricDatum;
use kitsune_p2p::event::MetricQuery;
use kitsune_p2p::event::MetricQueryAnswer;
use kitsune_p2p::event::TimeWindowMs;
use kitsune_p2p_types::tls::TlsConfig;
use std::future::Future;

/// Trait representing a persistence store.
pub trait AsKdPersist: 'static + Send + Sync {
    /// Get a uniq val that assists with Eq/Hash of trait objects.
    fn uniq(&self) -> Uniq;

    /// Check if this persist instance has been closed
    fn is_closed(&self) -> bool;

    /// Explicitly close this persist instance
    fn close(&self) -> BoxFuture<'static, ()>;

    /// Get or create and get the singleton tls cert creds for this store.
    fn singleton_tls_config(&self) -> BoxFuture<'static, KdResult<TlsConfig>>;

    /// Generate a signature keypair, returning the pub key as a KdHash.
    fn generate_signing_keypair(&self) -> BoxFuture<'static, KdResult<KdHash>>;

    /// Sign arbitrary data with the secret key associated with given KdHash.
    fn sign(&self, pub_key: KdHash, data: &[u8]) -> BoxFuture<'static, KdResult<Arc<[u8; 64]>>>;

    /// Store agent info
    fn store_agent_info(&self, agent_info: KdAgentInfo) -> BoxFuture<'static, KdResult<()>>;

    /// Get agent info
    fn get_agent_info(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> BoxFuture<'static, KdResult<KdAgentInfo>>;

    /// Query agent info
    fn query_agent_info(&self, root: KdHash) -> BoxFuture<'static, KdResult<Vec<KdAgentInfo>>>;

    /// Query agent info near basis
    fn query_agent_info_near_basis(
        &self,
        root: KdHash,
        basis_loc: u32,
        limit: u32,
    ) -> BoxFuture<'static, KdResult<Vec<KdAgentInfo>>>;

    /// Query the peer density of a space for a given [`DhtArc`].
    fn query_peer_density(
        &self,
        root: KdHash,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> BoxFuture<'static, KdResult<kitsune_p2p_types::dht_arc::PeerDensity>>;

    /// Store agent info
    fn put_metric_datum(&self, datum: MetricDatum) -> BoxFuture<'static, KdResult<()>>;

    /// Store agent info
    fn query_metrics(&self, query: MetricQuery) -> BoxFuture<'static, KdResult<MetricQueryAnswer>>;

    /// Store entry
    fn store_entry(
        &self,
        root: KdHash,
        agent: KdHash,
        entry: KdEntrySigned,
    ) -> BoxFuture<'static, KdResult<()>>;

    /// Get entry
    fn get_entry(
        &self,
        root: KdHash,
        agent: KdHash,
        hash: KdHash,
    ) -> BoxFuture<'static, KdResult<KdEntrySigned>>;

    /// Get entry
    fn query_entries(
        &self,
        root: KdHash,
        agent: KdHash,
        window_ms: TimeWindowMs,
        dht_arc: DhtArcSet,
    ) -> BoxFuture<'static, KdResult<Vec<KdEntrySigned>>>;

    /// Get ui file
    fn get_ui_file(&self, path: &str) -> BoxFuture<'static, KdResult<(String, Vec<u8>)>>;
}

/// Handle to a persistence store.
#[derive(Clone)]
pub struct KdPersist(pub Arc<dyn AsKdPersist>);

impl PartialEq for KdPersist {
    fn eq(&self, oth: &Self) -> bool {
        self.0.uniq().eq(&oth.0.uniq())
    }
}

impl Eq for KdPersist {}

impl std::hash::Hash for KdPersist {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.uniq().hash(state)
    }
}

impl KdPersist {
    /// Check if this persist instance has been closed
    pub fn is_closed(&self) -> bool {
        AsKdPersist::is_closed(&*self.0)
    }

    /// Explicitly close this persist instance
    pub fn close(&self) -> impl Future<Output = ()> + 'static + Send {
        AsKdPersist::close(&*self.0)
    }

    /// Get or create and get the singleton tls cert creds for this store.
    pub fn singleton_tls_config(
        &self,
    ) -> impl Future<Output = KdResult<TlsConfig>> + 'static + Send {
        AsKdPersist::singleton_tls_config(&*self.0)
    }

    /// Generate a signature keypair, returning the pub key as a KdHash.
    pub fn generate_signing_keypair(
        &self,
    ) -> impl Future<Output = KdResult<KdHash>> + 'static + Send {
        AsKdPersist::generate_signing_keypair(&*self.0)
    }

    /// Sign arbitrary data with the secret key associated with given KdHash.
    pub fn sign(
        &self,
        pub_key: KdHash,
        data: &[u8],
    ) -> impl Future<Output = KdResult<Arc<[u8; 64]>>> + 'static + Send {
        AsKdPersist::sign(&*self.0, pub_key, data)
    }

    /// Store agent info
    pub fn store_agent_info(
        &self,
        agent_info: KdAgentInfo,
    ) -> impl Future<Output = KdResult<()>> + 'static + Send {
        AsKdPersist::store_agent_info(&*self.0, agent_info)
    }

    /// Get agent info
    pub fn get_agent_info(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> impl Future<Output = KdResult<KdAgentInfo>> + 'static + Send {
        AsKdPersist::get_agent_info(&*self.0, root, agent)
    }

    /// Query agent info
    pub fn query_agent_info(
        &self,
        root: KdHash,
    ) -> impl Future<Output = KdResult<Vec<KdAgentInfo>>> + 'static + Send {
        AsKdPersist::query_agent_info(&*self.0, root)
    }

    /// Query agent info near basis
    pub fn query_agent_info_near_basis(
        &self,
        root: KdHash,
        basis_loc: u32,
        limit: u32,
    ) -> impl Future<Output = KdResult<Vec<KdAgentInfo>>> + 'static + Send {
        AsKdPersist::query_agent_info_near_basis(&*self.0, root, basis_loc, limit)
    }

    /// Query the peer density of a space for a given [`DhtArc`].
    pub fn query_peer_density(
        &self,
        root: KdHash,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> impl Future<Output = KdResult<kitsune_p2p_types::dht_arc::PeerDensity>> + 'static + Send
    {
        AsKdPersist::query_peer_density(&*self.0, root, dht_arc)
    }

    /// Store agent info
    pub fn store_metric_datum(
        &self,
        datum: MetricDatum,
    ) -> impl Future<Output = KdResult<()>> + 'static + Send {
        AsKdPersist::put_metric_datum(&*self.0, datum)
    }

    /// "Query" metric info
    pub async fn fetch_metrics(
        &self,
        query: MetricQuery,
    ) -> impl Future<Output = KdResult<MetricQueryAnswer>> + 'static + Send {
        AsKdPersist::query_metrics(&*self.0, query)
    }

    /// Store entry
    pub fn store_entry(
        &self,
        root: KdHash,
        agent: KdHash,
        entry: KdEntrySigned,
    ) -> impl Future<Output = KdResult<()>> + 'static + Send {
        AsKdPersist::store_entry(&*self.0, root, agent, entry)
    }

    /// Get entry
    pub fn get_entry(
        &self,
        root: KdHash,
        agent: KdHash,
        hash: KdHash,
    ) -> impl Future<Output = KdResult<KdEntrySigned>> + 'static + Send {
        AsKdPersist::get_entry(&*self.0, root, agent, hash)
    }

    /// Get entry
    pub fn query_entries(
        &self,
        root: KdHash,
        agent: KdHash,
        window: TimeWindowMs,
        dht_arc: DhtArcSet,
    ) -> impl Future<Output = KdResult<Vec<KdEntrySigned>>> + 'static + Send {
        AsKdPersist::query_entries(&*self.0, root, agent, window, dht_arc)
    }

    /// Get ui file
    pub fn get_ui_file(
        &self,
        path: &str,
    ) -> impl Future<Output = KdResult<(String, Vec<u8>)>> + 'static + Send {
        AsKdPersist::get_ui_file(&*self.0, path)
    }
}
