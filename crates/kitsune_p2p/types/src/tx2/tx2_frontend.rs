//! Abstraction traits / types for tx2 networking transport.

use crate::tx2::util::*;
use crate::tx2::*;
use crate::*;
use futures::future::BoxFuture;
use futures::stream::Stream;

/// Frontend Traits - you probably don't need these
/// unless you are implementing a custom tx2 frontend transport.
pub mod tx2_frontend_traits {
    use super::*;

    /// Trait representing a connection handle.
    pub trait AsConHnd: 'static + Send + Sync + Unpin {
        /// Is this connection closed?
        fn is_closed(&self) -> bool;

        /// Close this connection.
        fn close(&self, code: u32, reason: &str);

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
        /// Is this endpoint closed?
        fn is_closed(&self) -> bool;

        /// Close this endpoint.
        fn close(&self, code: u32, reason: &str);

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
#[derive(Clone)]
pub struct ConHnd(pub Arc<dyn AsConHnd>);

impl ConHnd {
    /// Is this connection closed?
    pub fn is_closed(&self) -> bool {
        AsConHnd::is_closed(self)
    }

    /// Close this connection.
    pub fn close(&self, code: u32, reason: &str) {
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
    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, code: u32, reason: &str) {
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

impl EpHnd {
    /// Is this endpoint closed?
    pub fn is_closed(&self) -> bool {
        AsEpHnd::is_closed(self)
    }

    /// Close this endpoint.
    pub fn close(&self, code: u32, reason: &str) {
        AsEpHnd::close(self, code, reason)
    }

    /// Get the bound local address of this endpoint.
    pub fn local_addr(&self) -> KitsuneResult<TxUrl> {
        AsEpHnd::local_addr(self)
    }

    /// Establish a new connection.
    pub fn connect(
        &self,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<ConHnd>> + 'static + Send {
        AsEpHnd::connect(self, remote, timeout)
    }
}

impl AsEpHnd for EpHnd {
    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, code: u32, reason: &str) {
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

/// Event emitted by a transport endpoint.
pub enum EpEvent {
    /// We've accepted an incoming connection.
    IncomingConnection(ConHnd),

    /// We've received incoming data on an open connection.
    IncomingData(ConHnd, MsgId, PoolBuf),

    /// A connection has closed (Url, Code, Reason).
    ConnectionClosed(TxUrl, u32, String),

    /// A non-fatal internal error.
    Error(KitsuneError),
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
    pub fn bind(
        &self,
        bind_spec: TxUrl,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<Ep>> + 'static + Send {
        AsEpFactory::bind(self, bind_spec, timeout)
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
