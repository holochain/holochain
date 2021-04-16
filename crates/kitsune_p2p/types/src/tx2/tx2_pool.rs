//! Abstraction traits / types for tx2 networking transport.

use crate::tx2::tx2_adapter::Uniq;
use crate::tx2::tx2_utils::*;
use crate::tx2::*;
use crate::*;
use futures::future::BoxFuture;
use futures::stream::Stream;

/// Trait representing a connection handle.
pub trait AsConHnd: std::fmt::Debug + 'static + Send + Sync + Unpin {
    /// Get the opaque Uniq identifier for this connection.
    fn uniq(&self) -> Uniq;

    /// Get the remote address of this connection.
    fn peer_addr(&self) -> KitsuneResult<TxUrl>;

    /// Get the certificate digest of the remote.
    fn peer_cert(&self) -> KitsuneResult<Tx2Cert>;

    /// Is this connection closed?
    fn is_closed(&self) -> bool;

    /// Close this connection.
    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()>;

    /// Write data to this connection.
    fn write(
        &self,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>>;
}

/// Trait object connection handle
pub type ConHnd = Arc<dyn AsConHnd>;

/// Trait representing a connection handle.
pub trait AsEpHnd: 'static + Send + Sync + Unpin {
    /// Capture a debugging internal state dump.
    fn debug(&self) -> serde_json::Value;

    /// Get the opaque Uniq identifier for this endpoint.
    fn uniq(&self) -> Uniq;

    /// Get the bound local address of this endpoint.
    fn local_addr(&self) -> KitsuneResult<TxUrl>;

    /// Get the local certificate digest.
    fn local_cert(&self) -> KitsuneResult<Tx2Cert>;

    /// Is this endpoint closed?
    fn is_closed(&self) -> bool;

    /// Close this endpoint.
    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()>;

    /// Force close a specific connection.
    fn close_connection(&self, remote: TxUrl, code: u32, reason: &str) -> BoxFuture<'static, ()>;

    /// Get a connection handle to an existing connection.
    /// If one does not exist, establish a new connection.
    fn get_connection(
        &self,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<ConHnd>>;

    /// Write data to target remote.
    fn write(
        &self,
        remote: TxUrl,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        let con_fut = self.get_connection(remote, timeout);
        futures::future::FutureExt::boxed(async move {
            con_fut.await?.write(msg_id, data, timeout).await
        })
    }
}

/// Trait object endpoint handle
pub type EpHnd = Arc<dyn AsEpHnd>;

/// Trait representing a transport endpoint.
pub trait AsEp: 'static + Send + Unpin + Stream<Item = EpEvent> {
    /// A cheaply clone-able handle to this endpoint.
    fn handle(&self) -> &EpHnd;
}

/// Trait object endpoint
pub type Ep = Box<dyn AsEp>;

/// Trait representing an endpoint factory (binder).
pub trait AsEpFactory: 'static + Send + Sync + Unpin {
    /// Bind a new local transport endpoint.
    fn bind(
        &self,
        bind_spec: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<Ep>>;
}

/// Trait object endpoint factory
pub type EpFactory = Arc<dyn AsEpFactory>;

/// Data associated with an IncomingConnection EpEvent
#[derive(Debug)]
pub struct EpConnection {
    /// handle to the remote connection
    pub con: ConHnd,

    /// the remote url for this connection
    pub url: TxUrl,
}

/// Data associated with an IncomingData EpEvent
#[derive(Debug)]
pub struct EpIncomingData {
    /// handle to the remote connection that send this data
    pub con: ConHnd,

    /// the remote url from which this data originated
    pub url: TxUrl,

    /// message_id associated with this incoming data
    pub msg_id: MsgId,

    /// the actual bytes of incoming data
    pub data: PoolBuf,
}

/// Data associated with an IncomingError EpEvent
#[derive(Debug)]
pub struct EpIncomingError {
    /// handle to the remote connection that send this data
    pub con: ConHnd,

    /// the remote url from which this data originated
    pub url: TxUrl,

    /// message_id associated with this incoming data
    pub msg_id: MsgId,

    /// the actual bytes of incoming data
    pub err: KitsuneError,
}

/// Data associated with a ConnectionClosed EpEvent
#[derive(Debug)]
pub struct EpConnectionClosed {
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
    /// We've established an outgoing connection.
    OutgoingConnection(EpConnection),

    /// We've accepted an incoming connection.
    IncomingConnection(EpConnection),

    /// We've received incoming data on an open connection.
    IncomingData(EpIncomingData),

    /// We've received incoming error on an open connection.
    IncomingError(EpIncomingError),

    /// A connection has closed (Url, Code, Reason).
    ConnectionClosed(EpConnectionClosed),

    /// A non-fatal internal error.
    Error(KitsuneError),

    /// The endpoint has closed.
    EndpointClosed,
}
