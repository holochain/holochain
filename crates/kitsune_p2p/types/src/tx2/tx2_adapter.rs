//! Types and Traits for writing tx2 adapters.

use crate::tx2::tx2_utils::TxUrl;
use crate::tx2::*;
use crate::*;

use futures::{future::BoxFuture, stream::Stream};
use std::sync::atomic;

static UNIQ: atomic::AtomicUsize = atomic::AtomicUsize::new(1);

/// Opaque identifier, allows Eq/Hash through trait-object types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Uniq(usize);

impl Default for Uniq {
    fn default() -> Self {
        Self(UNIQ.fetch_add(1, atomic::Ordering::Relaxed))
    }
}

/// The directionality of this connection.
/// Did we establish it? Was it an incoming connection?
#[derive(Debug, Clone, Copy)]
pub enum Tx2ConDir {
    /// This connection was initiated by a remote peer.
    Incoming,

    /// A local endpoint established this outgoing connection.
    Outgoing,
}

/// Tx backend read stream type.
pub type InChan = Box<dyn AsFramedReader>;

/// Tx backend future resolves to InChan instance.
pub type InChanFut = BoxFuture<'static, KitsuneResult<InChan>>;

/// Tx backend adapter for incoming InChan instances.
#[must_use = "streams do nothing unless polled"]
pub trait InChanRecvAdapt: 'static + Send + Unpin + Stream<Item = InChanFut> {}

/// Tx backend write stream type.
pub type OutChan = Box<dyn AsFramedWriter>;

/// Tx backend future resolves to OutChan type.
pub type OutChanFut = BoxFuture<'static, KitsuneResult<OutChan>>;

/// Tx backend adapter represents an open connection to a remote.
pub trait ConAdapt: 'static + Send + Sync + Unpin {
    /// Get the opaque Uniq identifier for this connection.
    fn uniq(&self) -> Uniq;

    /// Get the directionality of this connection.
    fn dir(&self) -> Tx2ConDir;

    /// Get the string address (url) of the remote.
    fn peer_addr(&self) -> KitsuneResult<TxUrl>;

    /// Get the certificate digest of the remote peer.
    fn peer_cert(&self) -> Tx2Cert;

    /// Create a new outgoing channel to the remote.
    fn out_chan(&self, timeout: KitsuneTimeout) -> OutChanFut;

    /// Check if this connection has closed.
    fn is_closed(&self) -> bool;

    /// Close this open connection (and all associated Chans).
    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()>;
}

/// A tx backend Con is both the ability to make outgoing channels,
/// but also to receive incoming channels.
pub type Con = (Arc<dyn ConAdapt>, Box<dyn InChanRecvAdapt>);

/// Tx backend future resolves to a Con instance.
pub type ConFut = BoxFuture<'static, KitsuneResult<Con>>;

/// Tx backend adapter for incoming Con instances.
#[must_use = "streams do nothing unless polled"]
pub trait ConRecvAdapt: 'static + Send + Unpin + Stream<Item = ConFut> {}

/// Tx backend adapter represents a bound local endpoint.
pub trait EndpointAdapt: 'static + Send + Sync + Unpin {
    /// Capture a debugging internal state dump.
    fn debug(&self) -> serde_json::Value;

    /// Get the opaque Uniq identifier for this endpoint.
    fn uniq(&self) -> Uniq;

    /// Get the string address (url) of this binding.
    fn local_addr(&self) -> KitsuneResult<TxUrl>;

    /// Get the local certificate digest.
    fn local_cert(&self) -> Tx2Cert;

    /// Create a new outgoing connection to a remote.
    fn connect(&self, url: TxUrl, timeout: KitsuneTimeout) -> ConFut;

    /// Check if this endpoint has closed.
    fn is_closed(&self) -> bool;

    /// Shutdown this endpoint / all connections / all channels.
    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()>;
}

/// A tx backend Endpoint is both the ability to make outgoing connections,
/// but also to receive incoming connections.
pub type Endpoint = (Arc<dyn EndpointAdapt>, Box<dyn ConRecvAdapt>);

/// Tx backend future resolves to an Endpoint instance.
pub type EndpointFut = BoxFuture<'static, KitsuneResult<Endpoint>>;

/// Tx bind adapter represents the ability to bind local endpoints.
pub trait BindAdapt: 'static + Send + Sync + Unpin {
    /// Bind a local endpoint, given a url spec.
    fn bind(&self, url: TxUrl, timeout: KitsuneTimeout) -> EndpointFut;
}

/// Tx backend endpoint binding factory type.
pub type AdapterFactory = Arc<dyn BindAdapt>;
