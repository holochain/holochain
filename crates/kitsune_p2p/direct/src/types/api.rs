//! kdirect api type

use crate::*;
use futures::future::BoxFuture;
use kitsune_p2p_direct_api::kd_entry::KdEntryBinary;
use std::future::Future;

/// Respond to an incoming Hello request
pub type HelloRespondCb =
    Box<dyn FnOnce(KdResult<KdEntryBinary>) -> BoxFuture<'static, KdResult<()>> + 'static + Send>;

/// Events emitted from a KdApi instance
pub enum KdApiEvt {
    /// A hello/authentication request from the server
    Hello {
        /// salt to use for the argon2id response
        salt: KdEntryBinary,

        /// respond with the pw hash generated with the given salt
        respond_cb: HelloRespondCb,
    },

    /// An incoming message from a remote node
    Message {
        /// the root app hash
        root: KdHash,

        /// the destination agent
        to_agent: KdHash,

        /// the source agent
        from_agent: KdHash,

        /// the structured content for this message
        content: serde_json::Value,

        /// the binary data associated with this message
        binary: KdEntryBinary,
    },
}

/// Stream of KdApiEvt instances
pub type KdApiEvtStream = Box<dyn futures::Stream<Item = KdApiEvt> + 'static + Send + Unpin>;

/// Trait representing a kitsune direct api implementation
pub trait AsKdApi: 'static + Send + Sync {
    /// Get a uniq val that assists with Eq/Hash of trait objects.
    fn uniq(&self) -> Uniq;

    /// Check if this kdirect instance has been closed
    fn is_closed(&self) -> bool;

    /// Explicitly close this kdirect instance
    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()>;

    /// Get or create a tagged keypair pub key hash
    fn keypair_get_or_create_tagged(&self, tag: &str) -> BoxFuture<'static, KdResult<KdHash>>;

    /// Join an agent to an app root hash
    fn app_join(&self, root: KdHash, agent: KdHash) -> BoxFuture<'static, KdResult<()>>;

    /// Inject an agent info record into the store from an outside source
    fn agent_info_store(&self, agent_info: KdAgentInfo) -> BoxFuture<'static, KdResult<()>>;

    /// get a specific agent_info record from the store
    fn agent_info_get(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> BoxFuture<'static, KdResult<KdAgentInfo>>;

    /// query a list of agent_info records from the store
    fn agent_info_query(&self, root: KdHash) -> BoxFuture<'static, KdResult<Vec<KdAgentInfo>>>;

    /// Send a message to a remote app/agent
    fn message_send(
        &self,
        root: KdHash,
        to_agent: KdHash,
        from_agent: KdHash,
        content: serde_json::Value,
        binary: KdEntryBinary,
    ) -> BoxFuture<'static, KdResult<()>>;

    /// Author / Publish a new KdEntry
    fn entry_author(
        &self,
        root: KdHash,
        author: KdHash,
        content: serde_json::Value,
        binary: KdEntryBinary,
    ) -> BoxFuture<'static, KdResult<KdEntrySigned>>;

    /// Get a specific entry
    fn entry_get(&self, root: KdHash, hash: KdHash) -> BoxFuture<'static, KdResult<KdEntrySigned>>;

    /// the result of the entry get children
    fn entry_get_children(
        &self,
        root: KdHash,
        parent: KdHash,
        kind: Option<String>,
    ) -> BoxFuture<'static, KdResult<Vec<KdEntrySigned>>>;
}

/// Struct representing a kitsune direct api implementation
#[derive(Clone)]
pub struct KdApi(pub Arc<dyn AsKdApi>);

impl PartialEq for KdApi {
    fn eq(&self, oth: &Self) -> bool {
        self.0.uniq().eq(&oth.0.uniq())
    }
}

impl Eq for KdApi {}

impl std::hash::Hash for KdApi {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.uniq().hash(state)
    }
}

impl KdApi {
    /// Check if this kdirect instance has been closed
    pub fn is_closed(&self) -> bool {
        AsKdApi::is_closed(&*self.0)
    }

    /// Explicitly close this kdirect instance
    pub fn close(&self, code: u32, reason: &str) -> impl Future<Output = ()> + 'static + Send {
        AsKdApi::close(&*self.0, code, reason)
    }

    /// Get or create a tagged keypair pub key hash
    pub fn keypair_get_or_create_tagged(
        &self,
        tag: &str,
    ) -> impl Future<Output = KdResult<KdHash>> + 'static + Send {
        AsKdApi::keypair_get_or_create_tagged(&*self.0, tag)
    }

    /// Join an agent to an app root hash
    pub fn app_join(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> impl Future<Output = KdResult<()>> + 'static + Send {
        AsKdApi::app_join(&*self.0, root, agent)
    }

    /// Inject an agent info record into the store from an outside source
    pub fn agent_info_store(
        &self,
        agent_info: KdAgentInfo,
    ) -> impl Future<Output = KdResult<()>> + 'static + Send {
        AsKdApi::agent_info_store(&*self.0, agent_info)
    }

    /// get a specific agent_info record from the store
    pub fn agent_info_get(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> impl Future<Output = KdResult<KdAgentInfo>> + 'static + Send {
        AsKdApi::agent_info_get(&*self.0, root, agent)
    }

    /// query a list of agent_info records from the store
    pub fn agent_info_query(
        &self,
        root: KdHash,
    ) -> impl Future<Output = KdResult<Vec<KdAgentInfo>>> + 'static + Send {
        AsKdApi::agent_info_query(&*self.0, root)
    }

    /// Send a message to a remote app/agent
    pub fn message_send(
        &self,
        root: KdHash,
        to_agent: KdHash,
        from_agent: KdHash,
        content: serde_json::Value,
        binary: KdEntryBinary,
    ) -> impl Future<Output = KdResult<()>> + 'static + Send {
        AsKdApi::message_send(&*self.0, root, to_agent, from_agent, content, binary)
    }

    /// Author / Publish a new KdEntry
    pub fn entry_author(
        &self,
        root: KdHash,
        author: KdHash,
        content: serde_json::Value,
        binary: KdEntryBinary,
    ) -> impl Future<Output = KdResult<KdEntrySigned>> + 'static + Send {
        AsKdApi::entry_author(&*self.0, root, author, content, binary)
    }

    /// Get a specific entry
    pub fn entry_get(
        &self,
        root: KdHash,
        hash: KdHash,
    ) -> impl Future<Output = KdResult<KdEntrySigned>> + 'static + Send {
        AsKdApi::entry_get(&*self.0, root, hash)
    }

    /// the result of the entry get children
    pub fn entry_get_children(
        &self,
        root: KdHash,
        parent: KdHash,
        kind: Option<String>,
    ) -> impl Future<Output = KdResult<Vec<KdEntrySigned>>> + 'static + Send {
        AsKdApi::entry_get_children(&*self.0, root, parent, kind)
    }
}
