//! Usability api for tx2 kitsune transports.

use crate::codec::*;
use crate::tx2::tx2_frontend::*;
use crate::tx2::tx2_utils::*;
use crate::tx2::*;
use crate::*;
use futures::future::{FutureExt, TryFutureExt};
use futures::stream::Stream;
use std::collections::HashMap;
use std::sync::atomic;

static MSG_ID: atomic::AtomicU64 = atomic::AtomicU64::new(1);

fn next_msg_id() -> u64 {
    MSG_ID.fetch_add(1, atomic::Ordering::Relaxed)
}

type RSend<C> = tokio::sync::oneshot::Sender<KitsuneResult<C>>;
type ShareRMap<C> = Arc<Share<RMap<C>>>;

struct RMap<C: Codec + 'static + Send + Unpin>(HashMap<(ConHnd, u64), (RSend<C>, KitsuneTimeout)>);

impl<C: Codec + 'static + Send + Unpin> RMap<C> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn insert(&mut self, con: ConHnd, msg_id: u64, s_res: RSend<C>, timeout: KitsuneTimeout) {
        self.0.insert((con, msg_id), (s_res, timeout));
    }

    pub fn respond(&mut self, con: ConHnd, msg_id: u64, c: C) {
        if let Some((s_res, _)) = self.0.remove(&(con, msg_id)) {
            // if the recv side is dropped, we no longer need to respond
            // so it's ok to ignore errors here.
            let _ = s_res.send(Ok(c));
        }
    }
}

/// Cleanup our map when the request future completes
/// either by recieving the response or timing out.
struct RMapDropCleanup<C: Codec + 'static + Send + Unpin>(ShareRMap<C>, ConHnd, u64);

impl<C: Codec + 'static + Send + Unpin> Drop for RMapDropCleanup<C> {
    fn drop(&mut self) {
        let _ = self.0.share_mut(|i, _| {
            i.0.remove(&(self.1.clone(), self.2));
            Ok(())
        });
    }
}

fn rmap_insert<C: Codec + 'static + Send + Unpin>(
    rmap: ShareRMap<C>,
    con: ConHnd,
    msg_id: u64,
    s_res: RSend<C>,
    timeout: KitsuneTimeout,
) -> KitsuneResult<RMapDropCleanup<C>> {
    let con2 = con.clone();
    rmap.share_mut(move |i, _| {
        i.insert(con2, msg_id, s_res, timeout);
        Ok(())
    })?;
    Ok(RMapDropCleanup(rmap, con, msg_id))
}

/// A connection handle - use this to manage an open connection.
#[derive(Clone)]
pub struct Tx2ConHnd<C: Codec + 'static + Send + Unpin>(ConHnd, ShareRMap<C>);

impl<C: Codec + 'static + Send + Unpin> std::fmt::Debug for Tx2ConHnd<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Tx2ConHnd").field(&self.0).finish()
    }
}

impl<C: Codec + 'static + Send + Unpin> Tx2ConHnd<C> {
    fn new(con: ConHnd, rmap: ShareRMap<C>) -> Self {
        Self(con, rmap)
    }
}

impl<C: Codec + 'static + Send + Unpin> PartialEq for Tx2ConHnd<C> {
    fn eq(&self, oth: &Self) -> bool {
        self.0.eq(&oth.0)
    }
}

impl<C: Codec + 'static + Send + Unpin> Eq for Tx2ConHnd<C> {}

impl<C: Codec + 'static + Send + Unpin> std::hash::Hash for Tx2ConHnd<C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<C: Codec + 'static + Send + Unpin> Tx2ConHnd<C> {
    /// Is this connection closed?
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    /// Close this connection.
    pub fn close(
        &self,
        code: u32,
        reason: &str,
    ) -> impl std::future::Future<Output = ()> + 'static + Send {
        self.0.close(code, reason)
    }

    /// Get the remote address of this connection.
    pub fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.remote_addr()
    }

    /// Write a request to this connection.
    pub fn request(
        &self,
        data: &C,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<C>> + 'static + Send {
        let mut buf = PoolBuf::new();
        if let Err(e) = data.encode(&mut buf) {
            return async move { Err(KitsuneError::other(e)) }.boxed();
        }
        let con = self.0.clone();
        let rmap = self.1.clone();
        async move {
            let msg_id = next_msg_id();
            let (s_res, r_res) = tokio::sync::oneshot::channel::<KitsuneResult<C>>();

            // insert our response receive handler
            // Cleanup our map when this future completes
            // either by recieving the response or timing out.
            let _drop_cleanup = rmap_insert(rmap, con.clone(), msg_id, s_res, timeout)?;

            con.write(MsgId::new(msg_id).as_req(), buf, timeout).await?;

            timeout.mix(r_res.map_err(KitsuneError::other)).await?
        }
        .boxed()
    }
}

/// An endpoint handle - use this to manage a bound endpoint.
#[derive(Clone)]
pub struct Tx2EpHnd<C: Codec + 'static + Send + Unpin>(EpHnd, ShareRMap<C>);

impl<C: Codec + 'static + Send + Unpin> Tx2EpHnd<C> {
    fn new(ep: EpHnd) -> Self {
        let rmap = Arc::new(Share::new(RMap::new()));
        Self(ep, rmap)
    }
}

impl<C: Codec + 'static + Send + Unpin> PartialEq for Tx2EpHnd<C> {
    fn eq(&self, oth: &Self) -> bool {
        self.0.eq(&oth.0)
    }
}

impl<C: Codec + 'static + Send + Unpin> Eq for Tx2EpHnd<C> {}

impl<C: Codec + 'static + Send + Unpin> std::hash::Hash for Tx2EpHnd<C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<C: Codec + 'static + Send + Unpin> Tx2EpHnd<C> {
    /// Capture a debugging internal state dump.
    pub fn debug(&self) -> serde_json::Value {
        self.0.debug()
    }

    /// Is this endpoint closed?
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    /// Close this endpoint.
    pub fn close(
        &self,
        code: u32,
        reason: &str,
    ) -> impl std::future::Future<Output = ()> + 'static + Send {
        self.0.close(code, reason)
    }

    /// Get the bound local address of this endpoint.
    pub fn local_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.local_addr()
    }

    /// Establish a new connection.
    pub fn connect<U: Into<TxUrl>>(
        &self,
        remote: U,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<Tx2ConHnd<C>>> + 'static + Send {
        let rmap = self.1.clone();
        self.0
            .connect(remote.into(), timeout)
            .map_ok(move |con| Tx2ConHnd::new(con, rmap))
    }
}

/// Data associated with an IncomingConnection EpEvent
#[derive(Debug)]
pub struct Tx2EpIncomingConnection<C: Codec + 'static + Send + Unpin> {
    /// the remote connection handle (could be closed)
    pub con: Tx2ConHnd<C>,

    /// the remote url from which this data originated
    /// this is included incase the con is closed
    pub url: TxUrl,
}

/// Respond to a Tx2EpIncomingRequest
pub struct Tx2Respond<C: Codec + 'static + Send + Unpin> {
    con: ConHnd,
    msg_id: u64,
    _p: std::marker::PhantomData<C>,
}

impl<C: Codec + 'static + Send + Unpin> std::fmt::Debug for Tx2Respond<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Tx2Respond").finish()
    }
}

impl<C: Codec + 'static + Send + Unpin> Tx2Respond<C> {
    fn new(con: ConHnd, msg_id: u64) -> Self {
        Self {
            con,
            msg_id,
            _p: std::marker::PhantomData,
        }
    }

    /// Respond to a Tx2EpIncomingRequest
    pub fn respond(
        self,
        data: C,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<()>> + 'static + Send {
        let Tx2Respond { con, msg_id, .. } = self;
        async move {
            let mut buf = PoolBuf::new();
            data.encode(&mut buf).map_err(KitsuneError::other)?;

            con.write(MsgId::new(msg_id).as_res(), buf, timeout).await
        }
    }
}

/// Data associated with an IncomingRequest EpEvent
#[derive(Debug)]
pub struct Tx2EpIncomingRequest<C: Codec + 'static + Send + Unpin> {
    /// the remote connection handle (could be closed)
    pub con: Tx2ConHnd<C>,

    /// the remote url from which this data originated
    /// this is included incase the con is closed
    pub url: TxUrl,

    /// the actual incoming message data
    pub data: C,

    /// callback for responding
    pub respond: Tx2Respond<C>,
}

/// Data associated with a ConnectionClosed EpEvent
#[derive(Debug)]
pub struct Tx2EpConnectionClosed<C: Codec + 'static + Send + Unpin> {
    /// the closed remote connection handle
    /// (can still use PartialEq/Hash)
    pub con: Tx2ConHnd<C>,

    /// the remote url this used to be connected to
    pub url: TxUrl,

    /// the code # indicating why the connection was closed
    pub code: u32,

    /// the human string reason this connection was closed
    pub reason: String,
}

/// Event emitted by a transport endpoint.
#[derive(Debug)]
pub enum Tx2EpEvent<C: Codec + 'static + Send + Unpin> {
    /// We've accepted an incoming connection.
    IncomingConnection(Tx2EpIncomingConnection<C>),

    /// We've received an incoming request on an open connection.
    IncomingRequest(Tx2EpIncomingRequest<C>),

    /// A connection has closed (Url, Code, Reason).
    ConnectionClosed(Tx2EpConnectionClosed<C>),

    /// A non-fatal internal error.
    Error(KitsuneError),

    /// We got an internal event...
    /// ignore this and poll again.
    Tick,

    /// The endpoint has closed.
    EndpointClosed,
}

/// Represents a bound endpoint. To manage this endpoint, see handle()/Tx2EpHnd.
/// To receive events from this endpoint, poll_next this instance as a Stream.
pub struct Tx2Ep<C: Codec + 'static + Send + Unpin>(Tx2EpHnd<C>, Ep);

impl<C: Codec + 'static + Send + Unpin> Stream for Tx2Ep<C> {
    type Item = Tx2EpEvent<C>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let rmap = self.0 .1.clone();
        let inner = &mut self.1;
        futures::pin_mut!(inner);
        match Stream::poll_next(inner, cx) {
            std::task::Poll::Ready(Some(evt)) => {
                let evt = match evt {
                    EpEvent::IncomingConnection(EpIncomingConnection { con, url }) => {
                        Tx2EpEvent::IncomingConnection(Tx2EpIncomingConnection {
                            con: Tx2ConHnd::new(con, rmap),
                            url,
                        })
                    }
                    EpEvent::IncomingData(EpIncomingData {
                        con,
                        url,
                        msg_id,
                        data,
                    }) => {
                        let (_, c) = match C::decode_ref(&data) {
                            Err(e) => {
                                // TODO - close connection?
                                return std::task::Poll::Ready(Some(Tx2EpEvent::Error(
                                    KitsuneError::other(e),
                                )));
                            }
                            Ok(c) => c,
                        };
                        match msg_id.get_type() {
                            MsgIdType::Notify => unimplemented!(),
                            MsgIdType::Req => Tx2EpEvent::IncomingRequest(Tx2EpIncomingRequest {
                                con: Tx2ConHnd::new(con.clone(), rmap),
                                url,
                                data: c,
                                respond: Tx2Respond::new(con, msg_id.as_id()),
                            }),
                            MsgIdType::Res => {
                                let _ = rmap.share_mut(move |i, _| {
                                    i.respond(con, msg_id.as_id(), c);
                                    Ok(())
                                });
                                Tx2EpEvent::Tick
                            }
                        }
                    }
                    EpEvent::ConnectionClosed(EpConnectionClosed {
                        con,
                        url,
                        code,
                        reason,
                    }) => Tx2EpEvent::ConnectionClosed(Tx2EpConnectionClosed {
                        con: Tx2ConHnd::new(con, rmap),
                        url,
                        code,
                        reason,
                    }),
                    EpEvent::Error(e) => Tx2EpEvent::Error(e),
                    EpEvent::EndpointClosed => Tx2EpEvent::EndpointClosed,
                };
                std::task::Poll::Ready(Some(evt))
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl<C: Codec + 'static + Send + Unpin> Tx2Ep<C> {
    /// A cheaply clone-able handle to this endpoint.
    pub fn handle(&self) -> &Tx2EpHnd<C> {
        &self.0
    }
}

/// Endpoint binding factory - lets us easily pass around logic
/// for later binding network transports.
pub struct Tx2EpFactory<C: Codec + 'static + Send + Unpin>(EpFactory, std::marker::PhantomData<C>);

impl<C: Codec + 'static + Send + Unpin> Tx2EpFactory<C> {
    /// Construct a new Tx2EpFactory instance from a frontend EpFactory
    pub fn new(factory: EpFactory) -> Self {
        Self(factory, std::marker::PhantomData)
    }

    /// Bind a new local transport endpoint.
    pub fn bind<U: Into<TxUrl>>(
        &self,
        bind_spec: U,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<Tx2Ep<C>>> + 'static + Send {
        self.0.bind(bind_spec.into(), timeout).map_ok(|ep| {
            let ep_hnd = ep.handle().clone();
            Tx2Ep(Tx2EpHnd::new(ep_hnd), ep)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx2::tx2_promote::*;
    use futures::stream::StreamExt;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx2_api() {
        let t = KitsuneTimeout::from_millis(5000);

        crate::write_codec_enum! {
            codec Test {
                One(0x01) {
                    data.0: usize,
                },
            }
        }

        fn handle(mut ep: Tx2Ep<Test>) -> tokio::task::JoinHandle<()> {
            tokio::task::spawn(async move {
                while let Some(evt) = ep.next().await {
                    if let Tx2EpEvent::IncomingRequest(Tx2EpIncomingRequest {
                        data, respond, ..
                    }) = evt
                    {
                        let val = match data {
                            Test::One(One { data }) => data,
                        };
                        let t = KitsuneTimeout::from_millis(5000);
                        respond.respond(Test::one(val + 1), t).await.unwrap();
                    }
                }
            })
        }

        let f = MemBackendAdapt::new();
        let f = tx2_promote(f, 32);
        let f = <Tx2EpFactory<Test>>::new(f);

        let ep1 = f.bind("none:", t).await.unwrap();
        let ep1_hnd = ep1.handle().clone();
        let ep1_task = handle(ep1);

        let ep2 = f.bind("none:", t).await.unwrap();
        let ep2_hnd = ep2.handle().clone();
        let ep2_task = handle(ep2);

        let addr2 = ep2_hnd.local_addr().unwrap();

        println!("addr2: {}", addr2);

        let con = ep1_hnd.connect(addr2, t).await.unwrap();
        let res = con.request(&Test::one(42), t).await.unwrap();

        assert_eq!(&Test::one(43), &res);

        ep1_hnd.close(0, "").await;
        ep2_hnd.close(0, "").await;

        ep1_task.await.unwrap();
        ep2_task.await.unwrap();
    }
}
