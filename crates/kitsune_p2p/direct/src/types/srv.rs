//! kdirect srv type

use crate::*;
use futures::future::BoxFuture;
use std::future::Future;

use kitsune_p2p_direct_api::KdApi;

/// HttpResponse data
pub struct HttpResponse {
    /// the response status code to send
    pub status: u16,

    /// the response body to send
    pub body: Vec<u8>,

    /// response headers
    pub headers: Vec<(String, Vec<u8>)>,
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self {
            status: 200,
            body: Vec::new(),
            headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        }
    }
}

/// Respond to an incoming http request
pub type HttpRespondCb = Box<
    dyn FnOnce(KitsuneResult<HttpResponse>) -> BoxFuture<'static, KitsuneResult<()>>
        + 'static
        + Send,
>;

/// Events emitted from a KdSrv instance.
pub enum KdSrvEvt {
    /// An incoming Http request
    HttpRequest {
        /// incoming uri
        uri: String,

        /// incoming method
        method: String,

        /// incoming headers
        headers: Vec<(String, Vec<u8>)>,

        /// incoming body
        body: Vec<u8>,

        /// response callback
        respond_cb: HttpRespondCb,
    },

    /// We've received a new incoming websocket connection
    WebsocketConnected {
        /// Connection Ref
        con: Uniq,
    },

    /// An incoming websocket json blob
    WebsocketMessage {
        /// Connection Ref
        con: Uniq,

        /// incoming structured message
        data: KdApi,
    },
}

/// Stream of KdSrvEvt instances
pub type KdSrvEvtStream = Box<dyn futures::Stream<Item = KdSrvEvt> + 'static + Send + Unpin>;

/// Trait representing a persistence store.
pub trait AsKdSrv: 'static + Send + Sync {
    /// Get a uniq val that assists with Eq/Hash of trait objects.
    fn uniq(&self) -> Uniq;

    /// Check if this persist instance has been closed
    fn is_closed(&self) -> bool;

    /// Explicitly close this persist instance
    fn close(&self) -> BoxFuture<'static, ()>;

    /// Get the bound addr of this KdSrv instance
    fn local_addr(&self) -> KitsuneResult<std::net::SocketAddr>;

    /// Broadcast to all connected websockets
    fn websocket_broadcast(&self, data: KdApi) -> BoxFuture<'static, KitsuneResult<()>>;

    /// Send data to a specific websocket connection
    fn websocket_send(&self, con: Uniq, data: KdApi) -> BoxFuture<'static, KitsuneResult<()>>;
}

/// Handle to a Srv instance.
#[derive(Clone)]
pub struct KdSrv(pub Arc<dyn AsKdSrv>);

impl PartialEq for KdSrv {
    fn eq(&self, oth: &Self) -> bool {
        self.0.uniq().eq(&oth.0.uniq())
    }
}

impl Eq for KdSrv {}

impl std::hash::Hash for KdSrv {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.uniq().hash(state)
    }
}

impl KdSrv {
    /// Check if this persist instance has been closed
    pub fn is_closed(&self) -> bool {
        AsKdSrv::is_closed(&*self.0)
    }

    /// Explicitly close this persist instance
    pub fn close(&self) -> impl Future<Output = ()> + 'static + Send {
        AsKdSrv::close(&*self.0)
    }

    /// Get the bound addr of this KdSrv instance
    pub fn local_addr(&self) -> KitsuneResult<std::net::SocketAddr> {
        AsKdSrv::local_addr(&*self.0)
    }

    /// Broadcast to all connected websockets
    pub fn websocket_broadcast(
        &self,
        data: KdApi,
    ) -> impl Future<Output = KitsuneResult<()>> + 'static + Send {
        AsKdSrv::websocket_broadcast(&*self.0, data)
    }

    /// Send data to a specific websocket connection
    pub fn websocket_send(
        &self,
        con: Uniq,
        data: KdApi,
    ) -> impl Future<Output = KitsuneResult<()>> + 'static + Send {
        AsKdSrv::websocket_send(&*self.0, con, data)
    }
}
