//! kdirect srv type

use crate::*;
use futures::future::BoxFuture;
use std::future::Future;

/// HttpResponse data
pub struct HttpResponse {
    /// the response status code to send
    pub status: u16,

    /// the response body to send
    pub body: Vec<u8>,

    /// response headers
    pub headers: Vec<(String, String)>,
}

/// Respond to an incoming http request
pub type HttpRespondCb = Box<dyn FnOnce(HttpResponse) -> BoxFuture<'static, KitsuneResult<()>> + 'static + Send>;

/// Events emitted from a KdSrv instance.
pub enum KdSrvEvt {
    /// An incoming Http request
    HttpRequest {
        /// incoming uri
        uri: String,

        /// incoming method
        method: String,

        /// incoming headers
        headers: Vec<(String, String)>,

        /// incoming body
        body: Vec<u8>,

        /// response callback
        respond_cb: HttpRespondCb,
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
}
