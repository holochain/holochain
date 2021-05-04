//! kdirect entrypoint type

use crate::*;
use futures::future::BoxFuture;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::future::Future;
use types::kdentry::KdEntry;
use types::kdhash::KdHash;

/// Events emitted from a kitsune direct instance.
pub enum KitsuneDirectEvt {
    /// A new agent was generated
    AgentGenerated {
        /// the pubkey of the generated agent
        hash: KdHash,
    },

    /// An agent join a root app
    Join {
        /// the root app
        root: KdHash,

        /// the agent
        agent: KdHash,
    },

    /// A message received from a remote instance
    Message {
        /// the root app
        root: KdHash,

        /// the source agent
        from_agent: KdHash,

        /// the destination agent
        to_agent: KdHash,

        /// the content of the message
        content: serde_json::Value,
    },

    /// A new entry was published locally
    EntryPublished {
        /// the root app
        root: KdHash,

        /// the entry that was published
        entry: KdEntry,
    },
}

/// Trait representing a kitsune direct api implementation
pub trait AsKitsuneDirect: 'static + Send + Sync {
    /// Get a uniq val that assists with Eq/Hash of trait objects.
    fn uniq(&self) -> Uniq;

    /// List transport bindings
    fn list_transport_bindings(&self) -> BoxFuture<'static, Vec<TxUrl>>;

    /// Create a new signature agent for use with kdirect
    fn generate_agent(&self) -> BoxFuture<'static, KitsuneResult<KdHash>>;

    /// Begin gossiping with given agent on given root app.
    fn join(&self, root: KdHash, agent: KdHash) -> BoxFuture<'static, KitsuneResult<()>>;

    /// Message an active agent
    fn message(
        &self,
        root: KdHash,
        from_agent: KdHash,
        to_agent: KdHash,
        content: serde_json::Value,
    ) -> BoxFuture<'static, KitsuneResult<()>>;

    /// Publish a new entry to given root app.
    fn publish_entry(&self, root: KdHash, entry: KdEntry) -> BoxFuture<'static, KitsuneResult<()>>;
}

/// Struct representing a kitsune direct api implementation
#[derive(Clone)]
pub struct KitsuneDirect(pub Arc<dyn AsKitsuneDirect>);

impl PartialEq for KitsuneDirect {
    fn eq(&self, oth: &Self) -> bool {
        self.0.uniq().eq(&oth.0.uniq())
    }
}

impl Eq for KitsuneDirect {}

impl std::hash::Hash for KitsuneDirect {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.uniq().hash(state)
    }
}

impl KitsuneDirect {
    /// List transport bindings
    pub fn list_transport_bindings(&self) -> impl Future<Output = Vec<TxUrl>> + 'static + Send {
        AsKitsuneDirect::list_transport_bindings(&*self.0)
    }

    /// Create a new signature agent for use with kdirect
    pub fn generate_agent(&self) -> impl Future<Output = KitsuneResult<KdHash>> + 'static + Send {
        AsKitsuneDirect::generate_agent(&*self.0)
    }

    /// Begin gossiping with given agent on given root app.
    pub fn join(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> impl Future<Output = KitsuneResult<()>> + 'static + Send {
        AsKitsuneDirect::join(&*self.0, root, agent)
    }

    /// Message an active agent
    pub fn message(
        &self,
        root: KdHash,
        from_agent: KdHash,
        to_agent: KdHash,
        content: serde_json::Value,
    ) -> impl Future<Output = KitsuneResult<()>> + 'static + Send {
        AsKitsuneDirect::message(&*self.0, root, from_agent, to_agent, content)
    }

    /// Publish a new entry to given root app.
    pub fn publish_entry(
        &self,
        root: KdHash,
        entry: KdEntry,
    ) -> impl Future<Output = KitsuneResult<()>> + 'static + Send {
        AsKitsuneDirect::publish_entry(&*self.0, root, entry)
    }
}
