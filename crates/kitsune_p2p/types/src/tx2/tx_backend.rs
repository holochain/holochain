//! Types and Traits for writing tx2 backends.

use crate::*;

use futures::future::BoxFuture;

///
pub type InChan = Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>;

///
pub type InChanFut = BoxFuture<'static, KitsuneResult<InChan>>;

///
pub type InChanFutFut<'a> = BoxFuture<'a, KitsuneResult<InChanFut>>;

///
pub trait InChanRecvAdapt: 'static + Send + Unpin {
    ///
    fn next(&mut self) -> InChanFutFut<'_>;
}

///
pub type OutChan = Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>;

///
pub type OutChanFut = BoxFuture<'static, KitsuneResult<OutChan>>;

///
pub trait ConAdapt: 'static + Send + Sync + Unpin {
    ///
    fn remote_addr(&self) -> KitsuneResult<String>;

    ///
    fn out_chan(&self, timeout: KitsuneTimeout) -> OutChanFut;

    ///
    fn close(&self) -> BoxFuture<'static, ()>;
}

///
pub type Con = (Arc<dyn ConAdapt>, Box<dyn InChanRecvAdapt>);

///
pub type ConFut = BoxFuture<'static, KitsuneResult<Con>>;

///
pub type ConFutFut<'a> = BoxFuture<'a, KitsuneResult<ConFut>>;

///
pub trait ConRecvAdapt: 'static + Send + Unpin {
    ///
    fn next(&mut self) -> ConFutFut<'_>;
}

///
pub trait EndpointAdapt: 'static + Send + Sync + Unpin {
    ///
    fn local_addr(&self) -> KitsuneResult<String>;

    ///
    fn connect(&self, url: String, timeout: KitsuneTimeout) -> ConFut;

    ///
    fn close(&self) -> BoxFuture<'static, ()>;
}

///
pub type Endpoint = (Arc<dyn EndpointAdapt>, Box<dyn ConRecvAdapt>);

///
pub type EndpointFut = BoxFuture<'static, KitsuneResult<Endpoint>>;

///
pub trait BackendAdapt: 'static + Send + Sync + Unpin {
    ///
    fn bind(&self, url: String, timeout: KitsuneTimeout) -> EndpointFut;
}
