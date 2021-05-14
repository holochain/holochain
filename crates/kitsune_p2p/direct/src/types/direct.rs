//! kdirect entrypoint type

use crate::*;
use futures::future::BoxFuture;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::future::Future;
use types::kdentry::KdEntry;
use types::kdhash::KdHash;
use types::persist::KdPersist;

/// Events emitted from a kitsune direct instance.
#[derive(Debug)]
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

/// Stream of KitsuneDirectEvt instances
pub type KitsuneDirectEvtStream =
    Box<dyn futures::Stream<Item = KitsuneDirectEvt> + 'static + Send + Unpin>;

/// Trait representing a kitsune direct api implementation
pub trait AsKitsuneDirect: 'static + Send + Sync {
    /// Get a uniq val that assists with Eq/Hash of trait objects.
    fn uniq(&self) -> Uniq;

    /// Check if this kdirect instance has been closed
    fn is_closed(&self) -> bool;

    /// Explicitly close this kdirect instance
    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()>;

    /// Get a handle to the persist store used by this kdirect instance.
    fn get_persist(&self) -> KdPersist;

    /// List transport bindings
    fn list_transport_bindings(&self) -> BoxFuture<'static, KitsuneResult<Vec<TxUrl>>>;

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
    fn publish_entry(
        &self,
        root: KdHash,
        agent: KdHash,
        entry: KdEntry,
    ) -> BoxFuture<'static, KitsuneResult<()>>;
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
    /// Check if this kdirect instance has been closed
    pub fn is_closed(&self) -> bool {
        AsKitsuneDirect::is_closed(&*self.0)
    }

    /// Explicitly close this kdirect instance
    pub fn close(&self, code: u32, reason: &str) -> impl Future<Output = ()> + 'static + Send {
        AsKitsuneDirect::close(&*self.0, code, reason)
    }

    /// Get a handle to the persist store used by this kdirect instance.
    /// (persist is closed separately, as we may have cleanup
    /// operations to do on the store.)
    pub fn get_persist(&self) -> KdPersist {
        AsKitsuneDirect::get_persist(&*self.0)
    }

    /// List transport bindings
    pub fn list_transport_bindings(
        &self,
    ) -> impl Future<Output = KitsuneResult<Vec<TxUrl>>> + 'static + Send {
        AsKitsuneDirect::list_transport_bindings(&*self.0)
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
        agent: KdHash,
        entry: KdEntry,
    ) -> impl Future<Output = KitsuneResult<()>> + 'static + Send {
        AsKitsuneDirect::publish_entry(&*self.0, root, agent, entry)
    }
}
