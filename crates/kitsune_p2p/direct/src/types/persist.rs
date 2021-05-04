//! kdirect persist type

use crate::*;
use futures::future::BoxFuture;
use std::future::Future;
use types::kdagent::*;
use types::kdhash::KdHash;

/// Trait representing a persistence store.
pub trait AsKdPersist: 'static + Send + Sync {
    /// Get a uniq val that assists with Eq/Hash of trait objects.
    fn uniq(&self) -> Uniq;

    /// Generate a signature keypair, returning the pub key as a KdHash.
    fn generate_signing_keypair(&self) -> BoxFuture<'static, KitsuneResult<KdHash>>;

    /// Sign arbitrary data with the secret key associated with given KdHash.
    fn sign(
        &self,
        pub_key: KdHash,
        data: &[u8],
    ) -> BoxFuture<'static, KitsuneResult<Arc<[u8; 64]>>>;

    /// Store agent info
    fn store_agent_info(&self, agent_info: KdAgentInfo) -> BoxFuture<'static, KitsuneResult<()>>;

    /// Get agent info
    fn get_agent_info(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> BoxFuture<'static, KitsuneResult<KdAgentInfo>>;

    /// Query agent info
    fn query_agent_info(&self, root: KdHash)
        -> BoxFuture<'static, KitsuneResult<Vec<KdAgentInfo>>>;
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
    /// Generate a signature keypair, returning the pub key as a KdHash.
    pub fn generate_signing_keypair(
        &self,
    ) -> impl Future<Output = KitsuneResult<KdHash>> + 'static + Send {
        AsKdPersist::generate_signing_keypair(&*self.0)
    }

    /// Sign arbitrary data with the secret key associated with given KdHash.
    pub fn sign(
        &self,
        pub_key: KdHash,
        data: &[u8],
    ) -> impl Future<Output = KitsuneResult<Arc<[u8; 64]>>> + 'static + Send {
        AsKdPersist::sign(&*self.0, pub_key, data)
    }

    /// Store agent info
    pub fn store_agent_info(
        &self,
        agent_info: KdAgentInfo,
    ) -> impl Future<Output = KitsuneResult<()>> + 'static + Send {
        AsKdPersist::store_agent_info(&*self.0, agent_info)
    }

    /// Get agent info
    pub fn get_agent_info(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> impl Future<Output = KitsuneResult<KdAgentInfo>> + 'static + Send {
        AsKdPersist::get_agent_info(&*self.0, root, agent)
    }

    /// Query agent info
    pub fn query_agent_info(
        &self,
        root: KdHash,
    ) -> impl Future<Output = KitsuneResult<Vec<KdAgentInfo>>> + 'static + Send {
        AsKdPersist::query_agent_info(&*self.0, root)
    }
}
