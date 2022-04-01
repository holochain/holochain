use std::sync::Arc;

/// KitsuneP2p Error Type.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KitsuneP2pError {
    /// GhostError
    #[error(transparent)]
    GhostError(#[from] ghost_actor::GhostError),

    /// Base Kitsune Error
    #[error(transparent)]
    KitsuneError(#[from] kitsune_p2p_types::KitsuneError),

    /// RoutingSpaceError
    #[error("Routing Space Error: {0:?}")]
    RoutingSpaceError(Arc<KitsuneSpace>),

    /// RoutingAgentError
    #[error("Routing Agent Error: {0:?}")]
    RoutingAgentError(Arc<KitsuneAgent>),

    /// DecodingError
    #[error("Decoding Error: {0}")]
    DecodingError(Box<str>),

    /// std::io::Error
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    /// Reqwest crate.
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    /// Bootstrap call failed.
    #[error("Bootstrap Error: {0}")]
    Bootstrap(Box<str>),

    /// SystemTime call failed.
    #[error(transparent)]
    SystemTime(#[from] std::time::SystemTimeError),

    /// Integer casting failed.
    #[error(transparent)]
    TryFromInt(#[from] std::num::TryFromIntError),

    /// Other
    #[error("Other: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

pub use crate::actor::KitsuneP2pResult;

impl KitsuneP2pError {
    /// promote a custom error type to a KitsuneP2pError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }

    /// generate a decoding error from a string
    pub fn decoding_error(s: String) -> Self {
        Self::DecodingError(s.into_boxed_str())
    }
}

impl From<String> for KitsuneP2pError {
    fn from(s: String) -> Self {
        #[derive(Debug, thiserror::Error)]
        struct OtherError(String);
        impl std::fmt::Display for OtherError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        KitsuneP2pError::other(OtherError(s))
    }
}

impl From<&str> for KitsuneP2pError {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

pub use kitsune_p2p_types::bin_types::*;

/// Data structures to be stored in the agent/peer database.
pub mod agent_store {
    pub use kitsune_p2p_types::agent_info::*;
}

pub mod actor;
pub mod event;
pub(crate) mod gossip;
#[allow(missing_docs)]
pub mod wire;

pub use gossip::GossipModuleType;
pub use kitsune_p2p_types::dht_arc;

#[allow(missing_docs)]
pub mod metrics;
