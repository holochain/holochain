#![deny(missing_docs)]
//! Types subcrate for kitsune-p2p.

/// Re-exported dependencies.
pub mod dependencies {
    pub use ::futures;
    pub use ::ghost_actor;
    pub use ::holochain_trace;
    pub use ::lair_keystore_api;
    pub use ::paste;
    pub use ::rustls;
    pub use ::serde;
    pub use ::serde_json;
    pub use ::thiserror;
    pub use ::tokio;
    pub use ::url2;

    #[cfg(feature = "fuzzing")]
    pub use ::proptest;
    #[cfg(feature = "fuzzing")]
    pub use ::proptest_derive;
}

use std::sync::Arc;

/// Typedef for result of `proc_count_now()`.
/// This value is on the scale of microseconds.
pub type ProcCountMicros = i64;

/// Monotonically nondecreasing process tick count, backed by tokio::time::Instant
/// as an i64 to facilitate reference times that may be less than the first
/// call to this function.
/// The returned value is on the scale of microseconds.
pub fn proc_count_now_us() -> ProcCountMicros {
    use once_cell::sync::Lazy;
    use tokio::time::Instant;
    static PROC_COUNT: Lazy<Instant> = Lazy::new(Instant::now);
    let r = *PROC_COUNT;
    Instant::now().saturating_duration_since(r).as_micros() as i64
}

/// Get the elapsed process count duration from a captured `ProcCount` to now.
/// If the duration would be negative, this fn returns a zero Duration.
pub fn proc_count_us_elapsed(pc: ProcCountMicros) -> std::time::Duration {
    let dur = proc_count_now_us() - pc;
    let dur = if dur < 0 { 0 } else { dur as u64 };
    std::time::Duration::from_micros(dur)
}

/// Helper function for the common case of returning this nested Unit type.
pub fn unit_ok_fut<E1, E2>() -> Result<MustBoxFuture<'static, Result<(), E2>>, E1> {
    use futures::FutureExt;
    Ok(async move { Ok(()) }.boxed().into())
}

/// Helper function for the common case of returning this boxed future type.
pub fn ok_fut<E1, R: Send + 'static>(result: R) -> Result<MustBoxFuture<'static, R>, E1> {
    use futures::FutureExt;
    Ok(async move { result }.boxed().into())
}

/// Helper function for the common case of returning this boxed future type.
pub fn box_fut_plain<'a, R: Send + 'a>(result: R) -> BoxFuture<'a, R> {
    use futures::FutureExt;
    async move { result }.boxed()
}

/// Helper function for the common case of returning this boxed future type.
pub fn box_fut<'a, R: Send + 'a>(result: R) -> MustBoxFuture<'a, R> {
    box_fut_plain(result).into()
}

use ::ghost_actor::dependencies::tracing;
use futures::future::BoxFuture;
use ghost_actor::dependencies::must_future::MustBoxFuture;

/// 32 byte binary TLS certificate digest.
pub type CertDigest = lair_keystore_api::encoding_types::BinDataSized<32>;

/// Extension trait for working with CertDigests.
pub trait CertDigestExt {
    /// Construct from a slice. Panicks if `slice.len() != 32`.
    fn from_slice(slice: &[u8]) -> Self;
}

impl CertDigestExt for CertDigest {
    fn from_slice(slice: &[u8]) -> Self {
        let mut out = [0; 32];
        out.copy_from_slice(slice);
        out.into()
    }
}

/// Error related to remote communication.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KitsuneErrorKind {
    /// Temp error type for internal logic.
    #[error("Unit")]
    Unit,

    /// The operation timed out.
    #[error("Operation timed out")]
    TimedOut(String),

    /// This object is closed, calls on it are invalid.
    #[error("This object is closed, calls on it are invalid.")]
    Closed,

    /// The operation is unauthorized by the host.
    #[error("Unauthorized")]
    Unauthorized,

    /// Bad external input.
    /// Can't proceed, but we don't have to shut everything down, either.
    #[error("Bad external input. Error: {0}  Input: {1}")]
    BadInput(Box<dyn std::error::Error + Send + Sync>, String),

    /// Unspecified error.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl PartialEq for KitsuneErrorKind {
    fn eq(&self, oth: &Self) -> bool {
        #[allow(clippy::match_like_matches_macro)]
        match (self, oth) {
            (Self::TimedOut(a), Self::TimedOut(b)) => a == b,
            (Self::Closed, Self::Closed) => true,
            _ => false,
        }
    }
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
    /// the "kind" of this KitsuneError
    pub fn kind(&self) -> &KitsuneErrorKind {
        &self.0
    }

    /// Create a bad_input error
    pub fn bad_input(e: impl Into<Box<dyn std::error::Error + Send + Sync>>, i: String) -> Self {
        Self(Arc::new(KitsuneErrorKind::BadInput(e.into(), i)))
    }

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

pub mod agent_info;
pub mod async_lazy;
pub mod bootstrap;
pub mod codec;
pub mod combinators;
pub mod config;
pub mod consistency;
pub mod fetch_pool;
pub mod metrics;
pub mod task_agg;
pub mod tls;
pub use kitsune_p2p_bin_data as bin_types;
pub mod tx_utils;

#[cfg(feature = "fixt")]
pub mod fixt;

pub use kitsune_p2p_dht as dht;
pub use kitsune_p2p_dht_arc as dht_arc;

/// KitsuneAgent in an Arc
pub type KAgent = Arc<bin_types::KitsuneAgent>;
/// KitsuneBasis in an Arc
pub type KBasis = Arc<bin_types::KitsuneBasis>;
/// KitsuneOpHash in an Arc
pub type KOpHash = Arc<bin_types::KitsuneOpHash>;
/// KitsuneSpace in an Arc
pub type KSpace = Arc<bin_types::KitsuneSpace>;
/// KitsuneOpData in an Arc
pub type KOpData = Arc<bin_types::KitsuneOpData>;
