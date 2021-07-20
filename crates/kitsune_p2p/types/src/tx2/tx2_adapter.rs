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
#[cfg_attr(feature = "test_utils", mockall::automock)]
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
#[cfg_attr(feature = "test_utils", mockall::automock)]
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
#[cfg_attr(feature = "test_utils", mockall::automock)]
pub trait BindAdapt: 'static + Send + Sync + Unpin {
    /// Bind a local endpoint, given a url spec.
    fn bind(&self, url: TxUrl, timeout: KitsuneTimeout) -> EndpointFut;
}

/// Tx backend endpoint binding factory type.
pub type AdapterFactory = Arc<dyn BindAdapt>;

/// mockall helper generators
#[cfg(feature = "test_utils")]
pub mod test_utils {
    use super::*;
    use futures::stream::BoxStream;

    /// generate an InChanRecvAdapt from a generic stream
    pub fn gen_mock_in_chan_recv_adapt(
        s: BoxStream<'static, InChanFut>,
    ) -> Box<dyn InChanRecvAdapt> {
        struct Out(BoxStream<'static, InChanFut>);
        impl futures::stream::Stream for Out {
            type Item = InChanFut;

            fn poll_next(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Self::Item>> {
                futures::stream::Stream::poll_next(std::pin::Pin::new(&mut self.0), cx)
            }
        }
        impl InChanRecvAdapt for Out {}

        Box::new(Out(s))
    }

    /// generate a ConRecvAdapt from a generic stream
    pub fn gen_mock_con_recv_adapt(s: BoxStream<'static, ConFut>) -> Box<dyn ConRecvAdapt> {
        struct Out(BoxStream<'static, ConFut>);
        impl futures::stream::Stream for Out {
            type Item = ConFut;

            fn poll_next(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Self::Item>> {
                futures::stream::Stream::poll_next(std::pin::Pin::new(&mut self.0), cx)
            }
        }
        impl ConRecvAdapt for Out {}

        Box::new(Out(s))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::tx2::tx2_utils::PoolBuf;
        use futures::future::FutureExt;
        use futures::stream::StreamExt;

        #[tokio::test]
        async fn test_mock_adapter_factory() {
            let gen_con = move || {
                let mut m = MockConAdapt::new();
                m.expect_out_chan().returning(move |_t| {
                    async move {
                        let mut m = MockAsFramedWriter::new();
                        m.expect_write().returning(move |_, buf, _| {
                            assert_eq!(b"test", buf.as_ref());
                            async move { Ok(()) }.boxed()
                        });
                        let out: OutChan = Box::new(m);
                        Ok(out)
                    }
                    .boxed()
                });
                let out: Arc<dyn ConAdapt> = Arc::new(m);
                out
            };

            let gen_in_chan_recv = move || {
                // return one result
                let mut m = MockAsFramedReader::new();
                let mut sent_one = false;
                m.expect_read().returning(move |_t| {
                    if sent_one {
                        futures::future::pending().boxed()
                    } else {
                        sent_one = true;
                        async move {
                            let mut buf = PoolBuf::new();
                            buf.extend_from_slice(b"test");
                            Ok((0.into(), buf))
                        }
                        .boxed()
                    }
                });
                let once: InChan = Box::new(m);
                let once = futures::stream::once(async move { async move { Ok(once) }.boxed() });
                // then return Poll::Pending forever
                let s = once.chain(futures::stream::pending());
                gen_mock_in_chan_recv_adapt(s.boxed())
            };

            let gen_ep = move || {
                let mut m = MockEndpointAdapt::new();
                m.expect_connect().returning(move |_, _| {
                    async move { Ok((gen_con(), gen_in_chan_recv())) }.boxed()
                });
                let out: Arc<dyn EndpointAdapt> = Arc::new(m);
                out
            };

            let gen_con_recv = move || {
                // return one result
                let once = (gen_con(), gen_in_chan_recv());
                let once = futures::stream::once(async move { async move { Ok(once) }.boxed() });
                // then return Poll::Pending forever
                let s = once.chain(futures::stream::pending());
                gen_mock_con_recv_adapt(s.boxed())
            };

            let mut m = MockBindAdapt::new();
            m.expect_bind()
                .returning(move |_, _| async move { Ok((gen_ep(), gen_con_recv())) }.boxed());
            let f = Arc::new(m);

            let t = KitsuneTimeout::from_millis(100);

            let (ep, mut con_recv) = f.bind("test://none".into(), t).await.unwrap();

            let (con, mut chan_recv) = con_recv.next().await.unwrap().await.unwrap();

            let mut chan_recv = chan_recv.next().await.unwrap().await.unwrap();

            let (_, buf) = chan_recv.read(t).await.unwrap();
            assert_eq!(b"test", buf.as_ref());

            let mut chan_send = con.out_chan(t).await.unwrap();
            let mut buf = PoolBuf::new();
            buf.extend_from_slice(b"test");
            chan_send.write(0.into(), buf, t).await.unwrap();

            let (con, mut chan_recv) = ep.connect("test://test".into(), t).await.unwrap();

            let mut chan_recv = chan_recv.next().await.unwrap().await.unwrap();

            let (_, buf) = chan_recv.read(t).await.unwrap();
            assert_eq!(b"test", buf.as_ref());

            let mut chan_send = con.out_chan(t).await.unwrap();
            let mut buf = PoolBuf::new();
            buf.extend_from_slice(b"test");
            chan_send.write(0.into(), buf, t).await.unwrap();
        }
    }
}
