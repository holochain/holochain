//! Types and Traits for writing tx2 backends.

use crate::*;

use futures::{future::BoxFuture, stream::BoxStream};

/// incoming data channel
pub type InboundChannel = Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>;

/// a future that resolves to an incoming data channel
pub type InboundChannelFut = BoxFuture<'static, KitsuneResult<InboundChannel>>;

/// stream of incoming data channels
pub type InboundChannelStream = BoxStream<'static, KitsuneResult<InboundChannelFut>>;

/// outbound data channel
pub type OutboundChannel = Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>;

/// a future that resolves to an outbound channel
pub type OutboundChannelFut = BoxFuture<'static, KitsuneResult<OutboundChannel>>;

/// represents a connection that can make outgoing channels
pub trait ConnectionBackend: 'static + Send + Unpin {
    /// create a new outgoing channel on this connection
    fn new_outbound(&self, timeout: KitsuneTimeout) -> OutboundChannelFut;
}

/// two halves of a logical connection (outgoing channels + incoming channels)
pub type ConnectionBackendPair = (Arc<dyn ConnectionBackend>, InboundChannelStream);

/// a future that resolves to a logical connection
pub type ConnectionBackendPairFut = BoxFuture<'static, KitsuneResult<ConnectionBackendPair>>;

/// represents an endpoint that can make outgoing connections
pub trait EndpointBackend: 'static + Send + Unpin {
    /// create a new outgoing connection from this endpoint
    fn connect(&self, url: String, timeout: KitsuneTimeout) -> ConnectionBackendPairFut;
}

/// a stream of incoming logical connection futures
pub type ConnectionStream = BoxStream<'static, KitsuneResult<ConnectionBackendPairFut>>;

/// two halves of a logical endpoint (outgoing connections + incoming connections)
pub type EndpointBackendPair = (Arc<dyn EndpointBackend>, ConnectionStream);

/// a futures that resolves to a logical endpoint
pub type EndpointBackendPairFut = BoxFuture<'static, KitsuneResult<EndpointBackendPair>>;

/// a factory that can establish endpoints
pub trait BindingBackend: 'static + Send + Unpin {
    /// establish a new endpoint on this system
    fn bind(&self, url: String, timeout: KitsuneTimeout) -> EndpointBackendPairFut;
}
