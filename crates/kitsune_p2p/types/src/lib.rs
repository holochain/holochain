#![deny(missing_docs)]
//! Types subcrate for kitsune-p2p.

/// Re-exported dependencies.
pub mod dependencies {
    pub use ::futures;
    pub use ::ghost_actor;
    pub use ::paste;
    pub use ::serde;
    pub use ::serde_json;
    pub use ::thiserror;
    pub use ::tokio;
    pub use ::url2;
}

use std::sync::Arc;

/// Error related to remote communication.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KitsuneErrorKind {
    /// Temp error type for internal logic.
    #[error("Unit")]
    Unit,

    /// The operation timed out.
    #[error("TimedOut")]
    TimedOut,

    /// This object is closed, calls on it are invalid.
    #[error("Closed")]
    Closed,

    /// Unspecified error.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

/// Error related to remote communication.
#[derive(Clone, Debug)]
pub struct KitsuneError(pub Arc<KitsuneErrorKind>);

impl std::fmt::Display for KitsuneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for KitsuneError {}

impl KitsuneError {
    /// promote a custom error type to a KitsuneError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self(Arc::new(KitsuneErrorKind::Other(e.into())))
    }
}

impl From<KitsuneErrorKind> for KitsuneError {
    fn from(k: KitsuneErrorKind) -> Self {
        Self(Arc::new(k))
    }
}

impl From<String> for KitsuneError {
    fn from(s: String) -> Self {
        #[derive(Debug, thiserror::Error)]
        struct OtherError(String);
        impl std::fmt::Display for OtherError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        KitsuneError::other(OtherError(s))
    }
}

impl From<&str> for KitsuneError {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl From<KitsuneError> for () {
    fn from(_: KitsuneError) {}
}

impl From<()> for KitsuneError {
    fn from(_: ()) -> Self {
        KitsuneErrorKind::Unit.into()
    }
}

/// Result type for remote communication.
pub type KitsuneResult<T> = Result<T, KitsuneError>;

mod timeout;
pub use timeout::*;

pub mod async_lazy;
mod auto_stream_select;
pub use auto_stream_select::*;
pub mod codec;
pub mod dht_arc;
pub mod metrics;
pub mod transport;
pub mod transport_mem;
pub mod transport_pool;
pub mod tx2;
