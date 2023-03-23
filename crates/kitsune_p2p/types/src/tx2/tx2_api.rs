#![allow(enum_intrinsics_non_enums)] // these actually *are* enums...
//! Usability api for tx2 kitsune transports.

use crate::codec::*;
use crate::tx2::tx2_adapter::Uniq;
use crate::tx2::tx2_pool::*;
use crate::tx2::tx2_utils::*;
use crate::tx2::*;
use crate::*;
use futures::future::{FutureExt, TryFutureExt};
use futures::stream::Stream;
use kitsune_p2p_block::BlockTargetId;
use std::collections::HashMap;
use std::sync::atomic;

static MSG_ID: atomic::AtomicU64 = atomic::AtomicU64::new(1);

fn next_msg_id() -> u64 {
    MSG_ID.fetch_add(1, atomic::Ordering::Relaxed)
}

type RSend<C> = tokio::sync::oneshot::Sender<KitsuneResult<C>>;
type ShareRMap<C> = Arc<Share<RMap<C>>>;

struct RMapItem<C: Codec + 'static + Send + Unpin> {
    sender: RSend<C>,
    start: tokio::time::Instant,
    timeout: std::time::Duration,
    dbg_name: &'static str,
    req_byte_count: usize,
    local_cert: Tx2Cert,
    peer_cert: Tx2Cert,
}

struct RMap<C: Codec + 'static + Send + Unpin>(HashMap<(Uniq, u64), RMapItem<C>>);

impl<C: Codec + 'static + Send + Unpin> RMap<C> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &mut self,
        uniq: Uniq,
        timeout: KitsuneTimeout,
        msg_id: u64,
        s_res: RSend<C>,
        dbg_name: &'static str,
        req_byte_count: usize,
        local_cert: Tx2Cert,
        peer_cert: Tx2Cert,
    ) {
        let timeout = timeout.time_remaining();
        self.0.insert(
            (uniq, msg_id),
            RMapItem {
                sender: s_res,
                start: tokio::time::Instant::now(),
                timeout,
                dbg_name,
                req_byte_count,
                local_cert,
                peer_cert,
            },
        );
    }

    pub fn respond(&mut self, uniq: Uniq, resp_byte_count: usize, msg_id: u64, c: C) {
        let resp_dbg_name = c.variant_type();
        if let Some(RMapItem {
            sender,
            start,
            timeout,
            dbg_name,
            req_byte_count,
            local_cert,
            peer_cert,
        }) = self.0.remove(&(uniq, msg_id))
        {
            let elapsed = start.elapsed();
            crate::metrics::metric_push_api_req_res_elapsed_ms(elapsed.as_millis() as u64);
            let elapsed_s = elapsed.as_secs_f64();

            tracing::trace!(
                %dbg_name,
                %req_byte_count,
                %resp_dbg_name,
                %resp_byte_count,
                ?local_cert,
                ?peer_cert,
                %elapsed_s,
                "(api) req success",
            );

            if elapsed_s / timeout.as_secs_f64() > 0.75 {
                tracing::warn!(
                    %dbg_name,
                    %req_byte_count,
                    %resp_dbg_name,
                    %resp_byte_count,
                    ?local_cert,
                    ?peer_cert,
                    %elapsed_s,
                    "(api) req approaching timeout (> 75%)",
                );
            }

            // if the recv side is dropped, we no longer need to respond
            // so it's ok to ignore errors here.
            let _ = sender.send(Ok(c));
        } else {
            tracing::warn!(
                %resp_dbg_name,
                %resp_byte_count,
                "(api) req UNMATCHED RESPONSE",
            );
        }
    }

    pub fn respond_err(&mut self, uniq: Uniq, msg_id: u64, err: KitsuneError) {
        if let Some(RMapItem {
            sender,
            start,
            dbg_name,
            req_byte_count,
            local_cert,
            peer_cert,
            ..
        }) = self.0.remove(&(uniq, msg_id))
        {
            let elapsed_s = start.elapsed().as_secs_f64();
            tracing::trace!(
                %dbg_name,
                %req_byte_count,
                ?local_cert,
                ?peer_cert,
                %elapsed_s,
                ?err,
                "(api) req err",
            );

            // if the recv side is dropped, we no longer need to respond
            // so it's ok to ignore errors here.
            let _ = sender.send(Err(err));
        }
    }
}

/// Cleanup our map when the request future completes
/// either by recieving the response or timing out.
struct RMapDropCleanup<C: Codec + 'static + Send + Unpin>(ShareRMap<C>, Uniq, u64);

impl<C: Codec + 'static + Send + Unpin> Drop for RMapDropCleanup<C> {
    fn drop(&mut self) {
        let _ = self.0.share_mut(|i, _| {
            if let Some(RMapItem {
                start,
                dbg_name,
                local_cert,
                peer_cert,
                ..
            }) = i.0.remove(&(self.1, self.2))
            {
                let elapsed_s = start.elapsed().as_secs_f64();
                tracing::warn!(
                    %dbg_name,
                    ?local_cert,
                    ?peer_cert,
                    %elapsed_s,
                    "(api) req dropped",
                );
            }
            Ok(())
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn rmap_insert<C: Codec + 'static + Send + Unpin>(
    rmap: ShareRMap<C>,
    uniq: Uniq,
    timeout: KitsuneTimeout,
    msg_id: u64,
    s_res: RSend<C>,
    dbg_name: &'static str,
    req_byte_count: usize,
    local_cert: Tx2Cert,
    peer_cert: Tx2Cert,
) -> KitsuneResult<RMapDropCleanup<C>> {
    rmap.share_mut(move |i, _| {
        i.insert(
            uniq,
            timeout,
            msg_id,
            s_res,
            dbg_name,
            req_byte_count,
            local_cert,
            peer_cert,
        );
        Ok(())
    })?;
    Ok(RMapDropCleanup(rmap, uniq, msg_id))
}

/// A connection handle - use this to manage an open connection.
#[derive(Clone)]
pub struct Tx2ConHnd<C: Codec + 'static + Send + Unpin> {
    local_cert: Tx2Cert,
    con: ConHnd,
    #[allow(dead_code)]
    url: TxUrl,
    rmap: ShareRMap<C>,
    metrics: Arc<Tx2ApiMetrics>,
}

impl<C: Codec + 'static + Send + Unpin> std::fmt::Debug for Tx2ConHnd<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Tx2ConHnd").field(&self.con).finish()
    }
}

impl<C: Codec + 'static + Send + Unpin> Tx2ConHnd<C> {
    fn new(
        local_cert: Tx2Cert,
        con: ConHnd,
        url: TxUrl,
        rmap: ShareRMap<C>,
        metrics: Arc<Tx2ApiMetrics>,
    ) -> Self {
        Self {
            local_cert,
            con,
            url,
            rmap,
            metrics,
        }
    }
}

impl<C: Codec + 'static + Send + Unpin> PartialEq for Tx2ConHnd<C> {
    fn eq(&self, oth: &Self) -> bool {
        self.con.uniq().eq(&oth.con.uniq())
    }
}

impl<C: Codec + 'static + Send + Unpin> Eq for Tx2ConHnd<C> {}

impl<C: Codec + 'static + Send + Unpin> std::hash::Hash for Tx2ConHnd<C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.con.uniq().hash(state)
    }
}

impl<C: Codec + 'static + Send + Unpin> Tx2ConHnd<C> {
    /// Get the opaque Uniq identifier for this connection.
    pub fn uniq(&self) -> Uniq {
        self.con.uniq()
    }

    /// Get the remote address of this connection.
    pub fn peer_addr(&self) -> KitsuneResult<TxUrl> {
        self.con.peer_addr()
    }

    /// Get the certificate digest of the remote.
    pub fn peer_cert(&self) -> Tx2Cert {
        self.con.peer_cert()
    }

    /// Is this connection closed?
    pub fn is_closed(&self) -> bool {
        self.con.is_closed()
    }

    /// Close this connection.
    pub fn close(
        &self,
        code: u32,
        reason: &str,
    ) -> impl std::future::Future<Output = ()> + 'static + Send {
        self.con.close(code, reason)
    }

    fn priv_notify(
        &self,
        data: PoolBuf,
        timeout: KitsuneTimeout,
        dbg_name: &'static str,
    ) -> impl std::future::Future<Output = KitsuneResult<()>> + 'static + Send {
        let this = self.clone();
        async move {
            let msg_id = MsgId::new_notify();
            let len = data.len();
            this.con.write(msg_id, data, timeout).await?;
            this.metrics.write_len(dbg_name, len);

            let peer_cert = this.peer_cert();
            tracing::trace!(
                %dbg_name,
                req_byte_count=%len,
                local_cert=?this.local_cert,
                ?peer_cert,
                "(api) notify",
            );

            Ok(())
        }
    }

    fn priv_request(
        &self,
        data: PoolBuf,
        timeout: KitsuneTimeout,
        dbg_name: &'static str,
    ) -> impl std::future::Future<Output = KitsuneResult<C>> + 'static + Send {
        let this = self.clone();
        async move {
            let msg_id = next_msg_id();
            let (s_res, r_res) = tokio::sync::oneshot::channel::<KitsuneResult<C>>();

            let peer_cert = this.peer_cert();

            let len = data.len();

            // insert our response receive handler
            // Cleanup our map when this future completes
            // either by recieving the response or timing out.
            let _drop_cleanup = rmap_insert(
                this.rmap.clone(),
                this.con.uniq(),
                timeout,
                msg_id,
                s_res,
                dbg_name,
                len,
                this.local_cert.clone(),
                peer_cert,
            )?;

            this.con
                .write(MsgId::new(msg_id).as_req(), data, timeout)
                .await?;

            this.metrics.write_len(dbg_name, len);

            timeout
                .mix(
                    "Tx2ConHnd::priv_request",
                    r_res.map_err(KitsuneError::other),
                )
                .await?
        }
    }

    /// Write a notify to this connection.
    pub fn notify(
        &self,
        data: &C,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<()>> + 'static + Send {
        let dbg_name = data.variant_type();
        let mut buf = PoolBuf::new();
        if let Err(e) = data.encode(&mut buf) {
            return async move { Err(KitsuneError::other(e)) }.boxed();
        }
        self.priv_notify(buf, timeout, dbg_name).boxed()
    }

    /// Write a request to this connection.
    pub fn request(
        &self,
        data: &C,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<C>> + 'static + Send {
        let dbg_name = data.variant_type();
        let mut buf = PoolBuf::new();
        if let Err(e) = data.encode(&mut buf) {
            return async move { Err(KitsuneError::other(e)) }.boxed();
        }
        self.priv_request(buf, timeout, dbg_name).boxed()
    }
}

/// An endpoint handle - use this to manage a bound endpoint.
#[derive(Clone)]
pub struct Tx2EpHnd<C: Codec + 'static + Send + Unpin>(
    EpHnd,
    ShareRMap<C>,
    Arc<Tx2ApiMetrics>,
    Tx2Cert,
);

impl<C: Codec + 'static + Send + Unpin> Tx2EpHnd<C> {
    fn new(local_cert: Tx2Cert, ep: EpHnd, metrics: Arc<Tx2ApiMetrics>) -> Self {
        let rmap = Arc::new(Share::new(RMap::new()));
        Self(ep, rmap, metrics, local_cert)
    }
}

impl<C: Codec + 'static + Send + Unpin> PartialEq for Tx2EpHnd<C> {
    fn eq(&self, oth: &Self) -> bool {
        self.0.uniq().eq(&oth.0.uniq())
    }
}

impl<C: Codec + 'static + Send + Unpin> Eq for Tx2EpHnd<C> {}

impl<C: Codec + 'static + Send + Unpin> std::hash::Hash for Tx2EpHnd<C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.uniq().hash(state);
    }
}

impl<C: Codec + 'static + Send + Unpin> std::fmt::Debug for Tx2EpHnd<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tx2EpHnd")
            .field("uniq", &self.0.uniq())
            .finish()
    }
}

impl<C: Codec + 'static + Send + Unpin> Tx2EpHnd<C> {
    /// Capture a debugging internal state dump.
    pub fn debug(&self) -> serde_json::Value {
        self.0.debug()
    }

    /// Get the opaque Uniq identifier for this endpoint.
    pub fn uniq(&self) -> Uniq {
        self.0.uniq()
    }

    /// Get the bound local address of this endpoint.
    pub fn local_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.local_addr()
    }

    /// Get the local certificate digest.
    pub fn local_cert(&self) -> Tx2Cert {
        self.0.local_cert()
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

    /// Force close a specific connection.
    pub fn close_connection(
        &self,
        remote: TxUrl,
        code: u32,
        reason: &str,
    ) -> impl std::future::Future<Output = ()> + 'static + Send {
        self.0.close_connection(remote, code, reason)
    }

    /// Get an existing connection.
    /// If one is not available, establish a new connection.
    pub fn get_connection<U: Into<TxUrl>>(
        &self,
        remote: U,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<Tx2ConHnd<C>>> + 'static + Send {
        let remote = remote.into();
        let rmap = self.1.clone();
        let metrics = self.2.clone();
        let local_cert = self.3.clone();
        let fut = self.0.get_connection(remote.clone(), timeout);
        async move {
            let con = fut.await?;
            Ok(Tx2ConHnd::new(local_cert, con, remote, rmap, metrics))
        }
    }

    /// Write a notify to this connection.
    pub fn notify<U: Into<TxUrl>>(
        &self,
        remote: U,
        data: &C,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<()>> + 'static + Send {
        let dbg_name = data.variant_type();
        let mut buf = PoolBuf::new();
        if let Err(e) = data.encode(&mut buf) {
            return async move { Err(KitsuneError::other(e)) }.boxed();
        }
        let con_fut = self.get_connection(remote.into(), timeout);
        futures::future::FutureExt::boxed(async move {
            con_fut.await?.priv_notify(buf, timeout, dbg_name).await
        })
    }

    /// Write a request to this connection.
    pub fn request<U: Into<TxUrl>>(
        &self,
        remote: U,
        data: &C,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<C>> + 'static + Send {
        let dbg_name = data.variant_type();
        let mut buf = PoolBuf::new();
        if let Err(e) = data.encode(&mut buf) {
            return async move { Err(KitsuneError::other(e)) }.boxed();
        }
        let con_fut = self.get_connection(remote.into(), timeout);
        futures::future::FutureExt::boxed(async move {
            con_fut.await?.priv_request(buf, timeout, dbg_name).await
        })
    }
}

/// Respond to a Tx2EpIncomingRequest
pub struct Tx2Respond<C: Codec + 'static + Send + Unpin> {
    local_cert: Tx2Cert,
    peer_cert: Tx2Cert,
    time: tokio::time::Instant,
    dbg_name: &'static str,
    req_byte_count: usize,
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
    fn new(
        local_cert: Tx2Cert,
        peer_cert: Tx2Cert,
        dbg_name: &'static str,
        req_byte_count: usize,
        con: ConHnd,
        msg_id: u64,
    ) -> Self {
        let time = tokio::time::Instant::now();
        Self {
            local_cert,
            peer_cert,
            time,
            dbg_name,
            req_byte_count,
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
        let Tx2Respond {
            local_cert,
            peer_cert,
            time,
            dbg_name,
            req_byte_count,
            con,
            msg_id,
            ..
        } = self;
        async move {
            let mut buf = PoolBuf::new();
            data.encode(&mut buf).map_err(KitsuneError::other)?;

            let elapsed_s = time.elapsed().as_secs_f64();
            let resp_dbg_name = data.variant_type();
            let resp_byte_count = buf.len();
            tracing::trace!(
                %dbg_name,
                %req_byte_count,
                %resp_dbg_name,
                %resp_byte_count,
                ?local_cert,
                ?peer_cert,
                %elapsed_s,
                "(api) res",
            );

            con.write(MsgId::new(msg_id).as_res(), buf, timeout).await
        }
    }
}

/// Data associated with an IncomingConnection EpEvent
#[derive(Debug)]
pub struct Tx2EpConnection<C: Codec + 'static + Send + Unpin> {
    /// the remote connection handle (could be closed)
    pub con: Tx2ConHnd<C>,

    /// the remote url from which this data originated
    /// this is included incase the con is closed
    pub url: TxUrl,
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

/// Data associated with an IncomingNotify EpEvent
#[derive(Debug)]
pub struct Tx2EpIncomingNotify<C: Codec + 'static + Send + Unpin> {
    /// the remote connection handle (could be closed)
    pub con: Tx2ConHnd<C>,

    /// the remote url from which this data originated
    /// this is included incase the con is closed
    pub url: TxUrl,

    /// the actual incoming message data
    pub data: C,
}

/// Data associated with a ConnectionClosed EpEvent
#[derive(Debug)]
pub struct Tx2EpConnectionClosed<C: Codec + 'static + Send + Unpin> {
    /// the remote connection handle (could be closed)
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
    /// We've established an incoming connection.
    OutgoingConnection(Tx2EpConnection<C>),

    /// We've accepted an incoming connection.
    IncomingConnection(Tx2EpConnection<C>),

    /// We've received an incoming request on an open connection.
    IncomingRequest(Tx2EpIncomingRequest<C>),

    /// We've received an incoming notification on an open connection.
    IncomingNotify(Tx2EpIncomingNotify<C>),

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
pub struct Tx2Ep<C: Codec + 'static + Send + Unpin>(Tx2EpHnd<C>, Ep, Arc<Tx2ApiMetrics>, Tx2Cert);

impl<C: Codec + 'static + Send + Unpin> Stream for Tx2Ep<C> {
    type Item = Tx2EpEvent<C>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let rmap = self.0 .1.clone();
        let local_cert = self.3.clone();
        let inner = &mut self.1;
        futures::pin_mut!(inner);
        match Stream::poll_next(inner, cx) {
            std::task::Poll::Ready(Some(evt)) => {
                let evt = match evt {
                    EpEvent::OutgoingConnection(EpConnection { con, url }) => {
                        Tx2EpEvent::OutgoingConnection(Tx2EpConnection {
                            con: Tx2ConHnd::new(local_cert, con, url.clone(), rmap, self.2.clone()),
                            url,
                        })
                    }
                    EpEvent::IncomingConnection(EpConnection { con, url }) => {
                        Tx2EpEvent::IncomingConnection(Tx2EpConnection {
                            con: Tx2ConHnd::new(local_cert, con, url.clone(), rmap, self.2.clone()),
                            url,
                        })
                    }
                    EpEvent::IncomingData(EpIncomingData {
                        con,
                        url,
                        msg_id,
                        data,
                    }) => {
                        let peer_cert = con.peer_cert();
                        let len = data.len();
                        let (_, c) = match C::decode_ref(&data) {
                            Err(e) => {
                                return std::task::Poll::Ready(Some(Tx2EpEvent::Error(
                                    KitsuneError::other(e),
                                )));
                            }
                            Ok(c) => c,
                        };
                        let dbg_name = c.variant_type();
                        match msg_id.get_type() {
                            MsgIdType::Notify => Tx2EpEvent::IncomingNotify(Tx2EpIncomingNotify {
                                con: Tx2ConHnd::new(
                                    local_cert,
                                    con.clone(),
                                    url.clone(),
                                    rmap,
                                    self.2.clone(),
                                ),
                                url,
                                data: c,
                            }),
                            MsgIdType::Req => Tx2EpEvent::IncomingRequest(Tx2EpIncomingRequest {
                                con: Tx2ConHnd::new(
                                    local_cert.clone(),
                                    con.clone(),
                                    url.clone(),
                                    rmap,
                                    self.2.clone(),
                                ),
                                url,
                                data: c,
                                respond: Tx2Respond::new(
                                    local_cert,
                                    peer_cert,
                                    dbg_name,
                                    len,
                                    con,
                                    msg_id.as_id(),
                                ),
                            }),
                            MsgIdType::Res => {
                                let _ = rmap.share_mut(move |i, _| {
                                    i.respond(con.uniq(), len, msg_id.as_id(), c);
                                    Ok(())
                                });
                                Tx2EpEvent::Tick
                            }
                        }
                    }
                    EpEvent::IncomingError(EpIncomingError {
                        con, msg_id, err, ..
                    }) => match msg_id.get_type() {
                        MsgIdType::Res => {
                            let _ = rmap.share_mut(move |i, _| {
                                i.respond_err(con.uniq(), msg_id.as_id(), err);
                                Ok(())
                            });
                            Tx2EpEvent::Tick
                        }
                        _ => {
                            // MAYBE - should this be a connection-specific
                            // error type, so we can give the con handle?
                            Tx2EpEvent::Error(err)
                        }
                    },
                    EpEvent::ConnectionClosed(EpConnectionClosed {
                        con,
                        url,
                        code,
                        reason,
                    }) => Tx2EpEvent::ConnectionClosed(Tx2EpConnectionClosed {
                        con: Tx2ConHnd::new(local_cert, con, url.clone(), rmap, self.2.clone()),
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

type WriteLenCb = Box<dyn Fn(&'static str, usize) + 'static + Send + Sync>;

/// Metrics callback manager to be injected into the endpoint
pub struct Tx2ApiMetrics {
    write_len: Option<WriteLenCb>,
}

impl Default for Tx2ApiMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Tx2ApiMetrics {
    /// Construct a new default Tx2ApiMetrics with no set callbacks
    pub fn new() -> Self {
        Self { write_len: None }
    }

    /// This callback will be invoked when we successfully write data
    /// to a transport connection.
    pub fn set_write_len<F>(mut self, f: F) -> Self
    where
        F: Fn(&'static str, usize) + 'static + Send + Sync,
    {
        let f: WriteLenCb = Box::new(f);
        self.write_len = Some(f);
        self
    }

    fn write_len(&self, d: &'static str, l: usize) {
        if let Some(cb) = &self.write_len {
            cb(d, l)
        }
    }
}

/// Construct a new Tx2EpFactory instance from a pool EpFactory
pub fn tx2_api<C: Codec + 'static + Send + Unpin>(
    factory: EpFactory,
    metrics: Tx2ApiMetrics,
) -> Tx2EpFactory<C> {
    Tx2EpFactory::new(factory, metrics)
}

/// Endpoint binding factory - lets us easily pass around logic
/// for later binding network transports.
pub struct Tx2EpFactory<C: Codec + 'static + Send + Unpin>(
    EpFactory,
    Arc<Tx2ApiMetrics>,
    std::marker::PhantomData<C>,
);

impl<C: Codec + 'static + Send + Unpin> Tx2EpFactory<C> {
    /// Construct a new Tx2EpFactory instance from a frontend EpFactory
    pub fn new(factory: EpFactory, metrics: Tx2ApiMetrics) -> Self {
        Self(factory, Arc::new(metrics), std::marker::PhantomData)
    }

    /// Bind a new local transport endpoint.
    pub fn bind<U: Into<TxUrl>>(
        &self,
        bind_spec: U,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<Tx2Ep<C>>> + 'static + Send {
        let metrics = self.1.clone();
        let fut = self.0.bind(bind_spec.into(), timeout);
        async move {
            let ep = fut.await?;
            let ep_hnd = ep.handle().clone();
            let local_cert = ep_hnd.local_cert();
            Ok(Tx2Ep(
                Tx2EpHnd::new(local_cert.clone(), ep_hnd, metrics.clone()),
                ep,
                metrics,
                local_cert,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx2::tx2_pool_promote::*;
    use futures::stream::StreamExt;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx2_api() {
        holochain_trace::test_run().ok();
        tracing::trace!("bob");

        let t = KitsuneTimeout::from_millis(5000);

        crate::write_codec_enum! {
            codec Test {
                One(0x01) {
                    data.0: usize,
                },
            }
        }

        fn handle(mut ep: Tx2Ep<Test>) -> tokio::task::JoinHandle<KitsuneResult<()>> {
            metric_task(async move {
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
                Ok(())
            })
        }

        let mk_ep = || async {
            let f = tx2_mem_adapter(MemConfig::default()).await.unwrap();
            let f = tx2_pool_promote(f, Default::default());
            let f = tx2_api(f, Default::default());

            f.bind("none:", t).await.unwrap()
        };

        let ep1 = mk_ep().await;
        let ep1_hnd = ep1.handle().clone();
        let ep1_task = handle(ep1);

        let ep2 = mk_ep().await;
        let ep2_hnd = ep2.handle().clone();
        let ep2_task = handle(ep2);

        let addr2 = ep2_hnd.local_addr().unwrap();

        println!("addr2: {}", addr2);

        let con = ep1_hnd.get_connection(addr2, t).await.unwrap();
        let res = con.request(&Test::one(42), t).await.unwrap();

        assert_eq!(&Test::one(43), &res);

        ep1_hnd.close(0, "").await;
        ep2_hnd.close(0, "").await;

        ep1_task.await.unwrap().unwrap();
        ep2_task.await.unwrap().unwrap();
    }
}
