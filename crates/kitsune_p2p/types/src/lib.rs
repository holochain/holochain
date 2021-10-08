#![deny(missing_docs)]
//! Types subcrate for kitsune-p2p.

/// Re-exported dependencies.
pub mod dependencies {
    pub use ::futures;
    pub use ::ghost_actor;
    pub use ::legacy_lair_api;
    pub use ::observability;
    pub use ::paste;
    pub use ::rustls;
    pub use ::serde;
    pub use ::serde_json;
    pub use ::thiserror;
    pub use ::tokio;
    pub use ::url2;
}

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

/// Helper function for the common case of returning this nested Unit type.
pub fn ok_fut<E1, R: Send + 'static>(result: R) -> Result<MustBoxFuture<'static, R>, E1> {
    use futures::FutureExt;
    Ok(async move { result }.boxed().into())
}

use ::ghost_actor::dependencies::tracing;
use ghost_actor::dependencies::must_future::MustBoxFuture;

pub use ::legacy_lair_api::actor::CertDigest;

/// Wrapper around CertDigest that provides some additional debugging helpers.
#[derive(Clone)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Tx2Cert(pub Arc<(CertDigest, String, String)>);

impl Tx2Cert {
    /// get the tls cert digest
    pub fn as_digest(&self) -> &CertDigest {
        self.as_ref()
    }

    /// get the cert bytes
    pub fn as_bytes(&self) -> &[u8] {
        self.as_ref()
    }

    /// get the base64 representation
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    /// get the base64 nickname
    pub fn as_nick(&self) -> &str {
        &self.0 .2
    }
}

impl std::fmt::Debug for Tx2Cert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cert(")?;
        f.write_str(self.as_nick())?;
        f.write_str(")")?;
        Ok(())
    }
}

impl PartialEq for Tx2Cert {
    fn eq(&self, oth: &Self) -> bool {
        self.0 .0.eq(&oth.0 .0)
    }
}

impl Eq for Tx2Cert {}

impl PartialOrd for Tx2Cert {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0 .0.partial_cmp(&other.0 .0)
    }
}

impl Ord for Tx2Cert {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0 .0.cmp(&other.0 .0)
    }
}

impl std::hash::Hash for Tx2Cert {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0 .0.hash(state);
    }
}

impl std::ops::Deref for Tx2Cert {
    type Target = CertDigest;

    fn deref(&self) -> &Self::Target {
        &self.0 .0
    }
}

impl std::convert::AsRef<CertDigest> for Tx2Cert {
    fn as_ref(&self) -> &CertDigest {
        std::ops::Deref::deref(self)
    }
}

impl std::convert::AsRef<[u8]> for Tx2Cert {
    fn as_ref(&self) -> &[u8] {
        &self.0 .0
    }
}

impl std::convert::AsRef<str> for Tx2Cert {
    fn as_ref(&self) -> &str {
        &self.0 .1
    }
}

impl From<Vec<u8>> for Tx2Cert {
    fn from(v: Vec<u8>) -> Self {
        let d: CertDigest = v.into();
        d.into()
    }
}

impl From<Arc<Vec<u8>>> for Tx2Cert {
    fn from(v: Arc<Vec<u8>>) -> Self {
        let d: CertDigest = v.into();
        d.into()
    }
}

impl From<CertDigest> for Tx2Cert {
    fn from(c: CertDigest) -> Self {
        let b64 = base64::encode_config(&**c, base64::URL_SAFE_NO_PAD);
        let nick = {
            let (start, _) = b64.split_at(6);
            let (_, end) = b64.split_at(b64.len() - 6);
            format!("{}..{}", start, end)
        };
        Self(Arc::new((c, b64, nick)))
    }
}

impl From<&CertDigest> for Tx2Cert {
    fn from(c: &CertDigest) -> Self {
        c.clone().into()
    }
}

impl From<Tx2Cert> for CertDigest {
    fn from(d: Tx2Cert) -> Self {
        d.0 .0.clone()
    }
}

impl From<&Tx2Cert> for CertDigest {
    fn from(d: &Tx2Cert) -> Self {
        d.0 .0.clone()
    }
}

use config::KitsuneP2pTuningParams;
use std::sync::Arc;

/// Error related to remote communication.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KitsuneErrorKind {
    /// Temp error type for internal logic.
    #[error("Unit")]
    Unit,

    /// The operation timed out.
    #[error("Operation timed out")]
    TimedOut,

    /// This object is closed, calls on it are invalid.
    #[error("This object is closed, calls on it are invalid.")]
    Closed,

    /// Unspecified error.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl PartialEq for KitsuneErrorKind {
    fn eq(&self, oth: &Self) -> bool {
        match self {
            Self::TimedOut => {
                if let Self::TimedOut = oth {
                    return true;
                }
            }
            Self::Closed => {
                if let Self::Closed = oth {
                    return true;
                }
            }
            _ => (),
        }
        false
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
mod auto_stream_select;
pub use auto_stream_select::*;
pub mod bin_types;
pub mod bootstrap;
pub mod codec;
pub mod combinators;
pub mod config;
pub mod consistency;
pub mod metrics;
pub mod reverse_semaphore;
pub mod task_agg;
pub mod tls;
pub mod transport;
pub mod transport_mem;
pub mod transport_pool;
pub mod tx2;

pub use kitsune_p2p_dht_arc as dht_arc;

use metrics::metric_task;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx2_digest() {
        let d: Tx2Cert = vec![0xdb; 32].into();
        println!("raw_debug: {:?}", d);
        println!("as_digest: {:?}", d.as_digest());
        println!("as_bytes: {:?}", d.as_bytes());
        println!("as_str: {:?}", d.as_str());
        println!("as_nick: {:?}", d.as_nick());
    }
}
