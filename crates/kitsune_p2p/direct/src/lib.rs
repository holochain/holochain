//! Kitsune P2p Direct - Kitsune P2p Application Demo

#![forbid(unsafe_code)]
#![forbid(warnings)]
#![forbid(missing_docs)]

use actor::KitsuneP2pSender;
use arc_swap::*;
use chrono::prelude::*;
use futures::FutureExt;
use kitsune_p2p::dependencies::kitsune_p2p_proxy;
use kitsune_p2p::dependencies::kitsune_p2p_types;
use kitsune_p2p::*;
use kitsune_p2p_types::codec::Codec;
use kitsune_p2p_types::dependencies::futures;
use kitsune_p2p_types::dependencies::ghost_actor as old_ghost_actor;
use kitsune_p2p_types::dependencies::url2;
use std::collections::HashMap;
use std::sync::Arc;
use url2::*;

/// re-exported dependencies
pub mod dependencies {
    pub use kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::futures;
    pub use sodoken;
}

mod error;
pub use error::*;

mod kd_entry;
pub use kd_entry::*;

mod kd_actor;
pub use kd_actor::KdHash;

/// Events receivable by activated acting_agents
#[derive(Debug)]
pub enum KdEvent {
    /// Send a message to another agent
    Message {
        /// the root agent/space for the destination
        root_agent: KdHash,

        /// the active destination agent
        to_active_agent: KdHash,

        /// the active source agent
        from_active_agent: KdHash,

        /// the content of the message
        content: serde_json::Value,
    },
}

/// Trait describing kitsune direct api.
pub trait AsKitsuneDirect: 'static + Send + Sync {
    /// List connection URLs
    fn list_transport_bindings(&self) -> ghost_actor::GhostFuture<Vec<Url2>, KdError>;

    /// Create a new signature agent for use with Kd
    fn generate_agent(&self) -> ghost_actor::GhostFuture<KdHash, KdError>;

    /// Sign data with internally managed private key associated
    /// with given pub key.
    fn sign(
        &self,
        pub_key: KdHash,
        data: sodoken::Buffer,
    ) -> ghost_actor::GhostFuture<Arc<[u8; 64]>, KdError>;

    /// Join space with given root agent / acting agent.
    /// The acting_agent will remain inactive until activated.
    fn join(
        &self,
        root_agent: KdHash,
        acting_agent: KdHash,
    ) -> ghost_actor::GhostFuture<(), KdError>;

    /// dump the current valid agent info database (for root_agent/space)
    fn list_known_agent_info(
        &self,
        root_agent: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<agent_store::AgentInfoSigned>, KdError>;

    /// inject agent info obtained from outside source
    fn inject_agent_info(
        &self,
        root_agent: KdHash,
        agent_info: Vec<agent_store::AgentInfoSigned>,
    ) -> ghost_actor::GhostFuture<(), KdError>;

    /// Activate a previously joined acting_agent
    fn activate(
        &self,
        acting_agent: KdHash,
    ) -> ghost_actor::GhostFuture<tokio::sync::mpsc::Receiver<KdEvent>, KdError>;

    /// Message an active agent
    fn message(
        &self,
        root_agent: KdHash,
        from_active_agent: KdHash,
        to_active_agent: KdHash,
        content: serde_json::Value,
    ) -> ghost_actor::GhostFuture<(), KdError>;

    /// Create entry
    fn create_entry(
        &self,
        root_agent: KdHash,
        by_agent: KdHash,
        new_entry: KdEntryBuilder,
    ) -> ghost_actor::GhostFuture<(), KdError>;

    /// List all nodes that have a left_link pointing to target.
    /// This will pass-through HSM nodes.
    fn list_left_links(
        &self,
        root_agent: KdHash,
        target: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<KdHash>, KdError>;

    ghost_actor::ghost_box_trait_fns!(AsKitsuneDirect);
}
ghost_actor::ghost_box_trait!(AsKitsuneDirect);

/// Kitsune direct handle type.
pub struct KitsuneDirect(pub Box<dyn AsKitsuneDirect>);
ghost_actor::ghost_box_new_type!(KitsuneDirect);

impl KitsuneDirect {
    /// List connection URLs
    pub fn list_transport_bindings(&self) -> ghost_actor::GhostFuture<Vec<Url2>, KdError> {
        AsKitsuneDirect::list_transport_bindings(&*self.0)
    }

    /// Create a new signature agent for use with Kd
    pub fn generate_agent(&self) -> ghost_actor::GhostFuture<KdHash, KdError> {
        AsKitsuneDirect::generate_agent(&*self.0)
    }

    /// Sign data with internally managed private key associated
    /// with given pub key.
    pub fn sign(
        &self,
        pub_key: KdHash,
        data: sodoken::Buffer,
    ) -> ghost_actor::GhostFuture<Arc<[u8; 64]>, KdError> {
        AsKitsuneDirect::sign(&*self.0, pub_key, data)
    }

    /// Join space with given root agent / acting agent.
    /// The acting_agent will remain inactive until activated.
    pub fn join(
        &self,
        root_agent: KdHash,
        acting_agent: KdHash,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        AsKitsuneDirect::join(&*self.0, root_agent, acting_agent)
    }

    /// dump the current valid agent info database (for root_agent/space)
    pub fn list_known_agent_info(
        &self,
        root_agent: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<agent_store::AgentInfoSigned>, KdError> {
        AsKitsuneDirect::list_known_agent_info(&*self.0, root_agent)
    }

    /// inject agent info obtained from outside source
    pub fn inject_agent_info(
        &self,
        root_agent: KdHash,
        agent_info: Vec<agent_store::AgentInfoSigned>,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        AsKitsuneDirect::inject_agent_info(&*self.0, root_agent, agent_info)
    }

    /// Activate a previously joined acting_agent
    pub fn activate(
        &self,
        acting_agent: KdHash,
    ) -> ghost_actor::GhostFuture<tokio::sync::mpsc::Receiver<KdEvent>, KdError> {
        AsKitsuneDirect::activate(&*self.0, acting_agent)
    }

    /// Message an active agent
    pub fn message(
        &self,
        root_agent: KdHash,
        from_active_agent: KdHash,
        to_active_agent: KdHash,
        content: serde_json::Value,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        AsKitsuneDirect::message(
            &*self.0,
            root_agent,
            from_active_agent,
            to_active_agent,
            content,
        )
    }

    /// Create entry
    pub fn create_entry(
        &self,
        root_agent: KdHash,
        by_agent: KdHash,
        new_entry: KdEntryBuilder,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        AsKitsuneDirect::create_entry(&*self.0, root_agent, by_agent, new_entry)
    }

    /// List all nodes that have a left_link pointing to target.
    /// This will pass-through HSM nodes.
    pub fn list_left_links(
        &self,
        root_agent: KdHash,
        target: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<KdHash>, KdError> {
        AsKitsuneDirect::list_left_links(&*self.0, root_agent, target)
    }
}

/// Kitsune P2p Direct Config
/// Most Kd config lives in the live persistance store,
/// but, to bootstrap, we need two things:
/// - the store path (or None if we shouldn't persist - i.e. for testing)
/// - the unlock passphrase to use for encrypting / decrypting persisted data
#[derive(Debug, Clone)]
pub struct KdConfig {
    /// Where to store the Kd persistence data on disk
    /// (None to not persist - will keep in memory - be wary of mem usage)
    pub persist_path: Option<std::path::PathBuf>,

    /// User supplied passphrase for encrypting persistance
    /// USE `sodoken::Buffer::new_memlocked()` TO KEEP SECURE!
    pub unlock_passphrase: sodoken::Buffer,

    /// Example directives:
    /// - "set_proxy_accept_all:"
    /// - "bind_mem_local:"
    /// - "bind_quic_local:kitsune-quic://0.0.0.0:0"
    /// - "bind_quic_proxy:kitsune-proxy://YADA.."
    pub directives: Vec<String>,
}

/// spawn a Kitsune P2p Direct actor
pub async fn spawn_kitsune_p2p_direct(config: KdConfig) -> KdResult<KitsuneDirect> {
    kd_actor::KdActor::new(config).await
}

#[cfg(test)]
mod test;
