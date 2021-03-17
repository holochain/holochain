//! Abstraction traits / types for tx2 networking transport.

use crate::tx2::tx2_backend::Uniq;
use crate::tx2::tx2_utils::*;
use crate::tx2::*;
use crate::*;
use futures::future::BoxFuture;
use futures::stream::Stream;

/// Frontend Traits - you probably don't need these
/// unless you are implementing a custom tx2 frontend transport.
pub mod tx2_frontend_traits {
    use super::*;

    /// Trait representing a connection handle.
    pub trait AsConHnd: std::fmt::Debug + 'static + Send + Sync + Unpin {
        /// Get the opaque Uniq identifier for this connection.
        fn uniq(&self) -> Uniq;

        /// Is this connection closed?
        fn is_closed(&self) -> bool;

        /// Close this connection.
        fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()>;

        /// Get the remote address of this connection.
        fn remote_addr(&self) -> KitsuneResult<TxUrl>;

        /// Write data to this connection.
        fn write(
            &self,
            msg_id: MsgId,
            data: PoolBuf,
            timeout: KitsuneTimeout,
        ) -> BoxFuture<'static, KitsuneResult<()>>;
    }

    /// Trait representing a connection handle.
    pub trait AsEpHnd: 'static + Send + Sync + Unpin {
        /// Get the opaque Uniq identifier for this endpoint.
        fn uniq(&self) -> Uniq;

        /// Is this endpoint closed?
        fn is_closed(&self) -> bool;

        /// Close this endpoint.
        fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()>;

        /// Get the bound local address of this endpoint.
        fn local_addr(&self) -> KitsuneResult<TxUrl>;

        /// Establish a new connection.
        fn connect(
            &self,
            remote: TxUrl,
            timeout: KitsuneTimeout,
        ) -> BoxFuture<'static, KitsuneResult<ConHnd>>;
    }

    /// Trait representing a transport endpoint.
    pub trait AsEp: 'static + Send + Unpin + Stream<Item = EpEvent> {
        /// A cheaply clone-able handle to this endpoint.
        fn handle(&self) -> &EpHnd;
    }

    /// Trait representing an endpoint factory (binder).
    pub trait AsEpFactory: 'static + Send + Sync + Unpin {
        /// Bind a new local transport endpoint.
        fn bind(
            &self,
            bind_spec: TxUrl,
            timeout: KitsuneTimeout,
        ) -> BoxFuture<'static, KitsuneResult<Ep>>;
    }
}

use tx2_frontend_traits::*;

/// A connection handle - use this to manage an open connection.
#[derive(Clone, Debug)]
pub struct ConHnd(pub Arc<dyn AsConHnd>);

impl PartialEq for ConHnd {
    fn eq(&self, oth: &Self) -> bool {
        self.uniq().eq(&oth.uniq())
    }
}

impl Eq for ConHnd {}

impl std::hash::Hash for ConHnd {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uniq().hash(state);
    }
}

impl ConHnd {
    /// Is this connection closed?
    pub fn is_closed(&self) -> bool {
        AsConHnd::is_closed(self)
    }

    /// Close this connection.
    pub fn close(
        &self,
        code: u32,
        reason: &str,
    ) -> impl std::future::Future<Output = ()> + 'static + Send {
        AsConHnd::close(self, code, reason)
    }

    /// Get the remote address of this connection.
    pub fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        AsConHnd::remote_addr(self)
    }

    /// Write data to this connection.
    pub fn write(
        &self,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<()>> + 'static + Send {
        AsConHnd::write(self, msg_id, data, timeout)
    }
}

impl AsConHnd for ConHnd {
    fn uniq(&self) -> Uniq {
        self.0.uniq()
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        self.0.close(code, reason)
    }

    fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.remote_addr()
    }

    fn write(
        &self,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        self.0.write(msg_id, data, timeout)
    }
}

/// An endpoint handle - use this to manage a bound endpoint.
#[derive(Clone)]
pub struct EpHnd(pub Arc<dyn AsEpHnd>);

impl PartialEq for EpHnd {
    fn eq(&self, oth: &Self) -> bool {
        self.uniq().eq(&oth.uniq())
    }
}

impl Eq for EpHnd {}

impl std::hash::Hash for EpHnd {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uniq().hash(state);
    }
}

impl EpHnd {
    /// Is this endpoint closed?
    pub fn is_closed(&self) -> bool {
        AsEpHnd::is_closed(self)
    }

    /// Close this endpoint.
    pub fn close(
        &self,
        code: u32,
        reason: &str,
    ) -> impl std::future::Future<Output = ()> + 'static + Send {
        AsEpHnd::close(self, code, reason)
    }

    /// Get the bound local address of this endpoint.
    pub fn local_addr(&self) -> KitsuneResult<TxUrl> {
        AsEpHnd::local_addr(self)
    }

    /// Establish a new connection.
    pub fn connect<U: Into<TxUrl>>(
        &self,
        remote: U,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<ConHnd>> + 'static + Send {
        AsEpHnd::connect(self, remote.into(), timeout)
    }
}

impl AsEpHnd for EpHnd {
    fn uniq(&self) -> Uniq {
        self.0.uniq()
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        self.0.close(code, reason)
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.local_addr()
    }

    fn connect(
        &self,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<ConHnd>> {
        self.0.connect(remote, timeout)
    }
}

/// Data associated with an IncomingConnection EpEvent
#[derive(Debug)]
pub struct EpIncomingConnection {
    /// the remote connection handle (could be closed)
    pub con: ConHnd,

    /// the remote url from which this data originated
    /// this is included incase the con is closed
    pub url: TxUrl,
}

/// Data associated with an IncomingData EpEvent
#[derive(Debug)]
pub struct EpIncomingData {
    /// the remote connection handle (could be closed)
    pub con: ConHnd,

    /// the remote url from which this data originated
    /// this is included incase the con is closed
    pub url: TxUrl,

    /// message_id associated with this incoming data
    pub msg_id: MsgId,

    /// the actual bytes of incoming data
    pub data: PoolBuf,
}

/// Data associated with a ConnectionClosed EpEvent
#[derive(Debug)]
pub struct EpConnectionClosed {
    /// the closed remote connection handle
    /// (can still use PartialEq/Hash)
    pub con: ConHnd,

    /// the remote url this used to be connected to
    pub url: TxUrl,

    /// the code # indicating why the connection was closed
    pub code: u32,

    /// the human string reason this connection was closed
    pub reason: String,
}

/// Event emitted by a transport endpoint.
#[derive(Debug)]
pub enum EpEvent {
    /// We've accepted an incoming connection.
    IncomingConnection(EpIncomingConnection),

    /// We've received incoming data on an open connection.
    IncomingData(EpIncomingData),

    /// A connection has closed (Url, Code, Reason).
    ConnectionClosed(EpConnectionClosed),

    /// A non-fatal internal error.
    Error(KitsuneError),

    /// The endpoint has closed.
    EndpointClosed,
}

/// Represents a bound endpoint. To manage this endpoint, see handle()/EpHnd.
/// To receive events from this endpoint, poll_next this instance as a Stream.
pub struct Ep(pub Box<dyn AsEp>);

impl Stream for Ep {
    type Item = EpEvent;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let inner = &mut self.0;
        futures::pin_mut!(inner);
        Stream::poll_next(inner, cx)
    }
}

impl Ep {
    /// A cheaply clone-able handle to this endpoint.
    pub fn handle(&self) -> &EpHnd {
        AsEp::handle(self)
    }
}

impl AsEp for Ep {
    fn handle(&self) -> &EpHnd {
        self.0.handle()
    }
}

/// Endpoint binding factory - lets us easily pass around logic
/// for later binding network transports.
pub struct EpFactory(pub Arc<dyn AsEpFactory>);

impl EpFactory {
    /// Bind a new local transport endpoint.
    pub fn bind<U: Into<TxUrl>>(
        &self,
        bind_spec: U,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<Ep>> + 'static + Send {
        AsEpFactory::bind(self, bind_spec.into(), timeout)
    }
}

impl AsEpFactory for EpFactory {
    fn bind(
        &self,
        bind_spec: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<Ep>> {
        self.0.bind(bind_spec, timeout)
    }
}
