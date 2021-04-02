#![allow(clippy::new_ret_no_self)]
#![allow(clippy::manual_async_fn)]
//! Promote a tx2 transport backend to a tx2 transport frontend.

use crate::tx2::tx2_backend::*;
use crate::tx2::tx2_pool::*;
use crate::tx2::tx2_utils::*;
use crate::tx2::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt, Shared};
use futures::stream::{Stream, StreamExt};
use ghost_actor::dependencies::tracing;
use std::collections::HashMap;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Promote a tx2 transport backend to a tx2 transport frontend.
pub fn tx2_pool_promote(backend: BackendFactory, max_cons: usize) -> EpFactory {
    Arc::new(PromoteFactory { max_cons, backend })
}

// -- private -- //

/// Limit the number of active channels allowed per connection.
const CHANNELS_PER_CONNECTION: usize = 3;

/// Timeout after which we will give up trying to read from a channel.
const MAX_READ_TIMEOUT: u64 = 1000 * 30;

struct WriteChan {
    _permit: OwnedSemaphorePermit,
    writer: OutChan,
}

struct ConItemInner {
    _permit: OwnedSemaphorePermit,
    inner: Share<PromoteEpInner>,
    con: Arc<dyn ConAdapt>,
    url: TxUrl,
    writer_bucket: ResourceBucket<WriteChan>,
}

impl ConItemInner {
    pub fn close(
        &self,
        uniq: Uniq,
        code: u32,
        reason: &str,
    ) -> impl std::future::Future<Output = ()> + 'static + Send {
        let _ = self.inner.share_mut(|i, _| {
            let u = match i.cons.get(&self.url) {
                None => return Ok(()),
                Some(ci) => ci.uniq,
            };
            if u == uniq {
                i.cons.remove(&self.url);
            }
            Ok(())
        });
        self.con.close(code, reason)
    }
}

async fn in_chan_recv_logic(
    url: TxUrl,
    con_item: ConItem,
    writer_bucket: ResourceBucket<WriteChan>,
    logic_hnd: LogicChanHandle<EpEvent>,
    in_chan_recv: Box<dyn InChanRecvAdapt>,
) {
    let url = &url;
    let con_item = &con_item;
    let logic_hnd = &logic_hnd;

    let recv_fut = in_chan_recv
        .for_each_concurrent(CHANNELS_PER_CONNECTION, move |chan| async move {
            let mut chan = match chan.await {
                Err(e) => {
                    // unable to resolve incoming channel
                    // shut down the connection

                    let reason = format!("{:?}", e);

                    // TODO - standardize codes?
                    con_item.close(500, &reason).await;

                    // exit the loop
                    return;
                }
                Ok(c) => c,
            };
            loop {
                let r = chan
                    .read(KitsuneTimeout::from_millis(MAX_READ_TIMEOUT))
                    .await;

                let (msg_id, data) = match r {
                    Err(e) if *e.kind() == KitsuneErrorKind::Closed => {
                        // this channel was closed - exit the loop
                        // allowing another channel to be processed.
                        return;
                    }
                    Err(e) => {
                        // unrecoverable error - shut down the connection

                        let reason = format!("{:?}", e);

                        // TODO - standardize codes?
                        con_item.close(500, &reason).await;

                        // exit the loop
                        return;
                    }
                    Ok(r) => r,
                };

                let con: ConHnd = Arc::new(con_item.clone());
                let data = EpIncomingData {
                    con,
                    url: url.clone(),
                    msg_id,
                    data,
                };

                if logic_hnd.emit(EpEvent::IncomingData(data)).await.is_err() {
                    // the only reason this will error is if our
                    // endpoint is shut down, in which case we
                    // no longer care about the error.
                    break;
                }
            }
        })
        .boxed();

    let write_fut = async move {
        let limit = Arc::new(Semaphore::new(CHANNELS_PER_CONNECTION));
        loop {
            let permit = match limit.clone().acquire_owned().await {
                Err(_) => {
                    // we only get errors here when our endpoint has closed
                    // we can safely just exit this loop
                    return;
                }
                Ok(p) => p,
            };

            let writer = match con_item
                .out_chan(KitsuneTimeout::from_millis(MAX_READ_TIMEOUT))
                .await
            {
                Err(e) => {
                    // we were not able to create an outgoing channel
                    // clean up the connection.

                    let reason = format!("{:?}", e);

                    // TODO - standardize codes?
                    con_item.close(500, &reason).await;

                    // exit the loop
                    return;
                }
                Ok(c) => c,
            };

            writer_bucket.release(WriteChan {
                _permit: permit,
                writer,
            });
        }
    };

    // We can ignore errors, as they only happen on shutdown of the endpoint.
    let _ = futures::future::join(recv_fut, write_fut).await;
}

#[derive(Clone)]
struct ConItem {
    pub uniq: Uniq,
    pub item: Share<ConItemInner>,
}

impl std::fmt::Debug for ConItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ConHnd").field(&self.uniq).finish()
    }
}

impl AsConHnd for ConItem {
    fn uniq(&self) -> Uniq {
        self.uniq
    }

    fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        self.item.share_mut(|i, _| Ok(i.url.clone()))
    }

    fn is_closed(&self) -> bool {
        self.item.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        let maybe = self.item.share_mut(|i, c| {
            *c = true;
            Ok(i.close(self.uniq, code, reason))
        });
        async move {
            if let Ok(close_fut) = maybe {
                close_fut.await;
            }
        }
        .boxed()
    }

    fn write(
        &self,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        let this = self.clone();
        async move {
            let mut writer = this
                .item
                .share_mut(|i, _| Ok(i.writer_bucket.acquire(Some(timeout))))?
                .await?;

            if let Err(e) = writer.writer.write(msg_id, data, timeout).await {
                let reason = format!("{:?}", e);
                // TODO - standardize codes?
                this.close(500, &reason).await;
                return Err(e);
            }

            this.item.share_mut(move |i, _| {
                i.writer_bucket.release(writer);
                Ok(())
            })
        }
        .boxed()
    }
}

impl ConItem {
    pub async fn out_chan(&self, t: KitsuneTimeout) -> KitsuneResult<OutChan> {
        self.item.share_mut(|i, _| Ok(i.con.out_chan(t)))?.await
    }

    // register this connection
    pub async fn reg_con_inner(
        inner: Share<PromoteEpInner>,
        permit: OwnedSemaphorePermit,
        con: Arc<dyn ConAdapt>,
        url: TxUrl,
        in_chan_recv: Box<dyn InChanRecvAdapt>,
        is_outgoing: bool,
    ) -> KitsuneResult<Self> {
        let uniq = con.uniq();

        let writer_bucket = ResourceBucket::new();

        let con_item = Share::new(ConItemInner {
            _permit: permit,
            inner: inner.clone(),
            con,
            url: url.clone(),
            writer_bucket: writer_bucket.clone(),
        });

        let con_item = Self {
            uniq,
            item: con_item,
        };

        // move us to the full cons list
        let logic_hnd = inner.share_mut(|i, _| {
            i.pend_cons.remove(&url);
            i.cons.insert(url.clone(), con_item.clone());
            Ok(i.logic_hnd.clone())
        })?;

        // This is a calculated task spawn.
        // We could let this future be polled in the top-level endpoint
        // logic channel, but then our system would be single threaded.
        // Instead, we spawn one task per connection, gathering
        // the incoming channel data to be processed.
        tokio::task::spawn(in_chan_recv_logic(
            url.clone(),
            con_item.clone(),
            writer_bucket,
            logic_hnd.clone(),
            in_chan_recv,
        ));

        if is_outgoing {
            let _ = logic_hnd.emit(EpEvent::OutgoingConnection(EpConnection {
                con: Arc::new(con_item.clone()),
                url,
            })).await;
        } else {
            let _ = logic_hnd.emit(EpEvent::IncomingConnection(EpConnection {
                con: Arc::new(con_item.clone()),
                url,
            })).await;
        }

        Ok(con_item)
    }

    // The raw fallible inner future
    // `inner_connect` must catch errors to delete the entry from pend_cons
    pub fn inner_con_inner(
        inner: Share<PromoteEpInner>,
        con_limit: Arc<Semaphore>,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<Self>> {
        timeout.mix(async move {
            let permit = con_limit
                .acquire_owned()
                .await
                .map_err(KitsuneError::other)?;

            let mut next_wait_ms = 200;

            let try_connect = || async {
                inner
                    .share_mut(|i, _| Ok(i.sub_ep.connect(remote.clone(), timeout)))?
                    .await
            };

            loop {
                match try_connect().await {
                    Err(e) => tracing::warn!("connect error: {:?}", e),
                    Ok((con, in_chan_recv)) => {
                        return Self::reg_con_inner(inner, permit, con, remote, in_chan_recv, true).await;
                    }
                }

                tokio::time::sleep(std::time::Duration::from_millis(next_wait_ms)).await;

                next_wait_ms *= 2;
                if next_wait_ms >= timeout.time_remaining().as_millis() as u64 {
                    return Err(KitsuneErrorKind::TimedOut.into());
                }
            }
        })
    }

    // Build the future that goes in pend_cons
    fn inner_con(
        inner: Share<PromoteEpInner>,
        con_limit: Arc<Semaphore>,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> Shared<BoxFuture<'static, KitsuneResult<Self>>> {
        async move {
            match Self::inner_con_inner(inner.clone(), con_limit, remote.clone(), timeout).await {
                Ok(con_item) => Ok(con_item),
                Err(e) => {
                    // remove the pending entry
                    let _ = inner.share_mut(|i, _| {
                        i.pend_cons.remove(&remote);
                        Ok(())
                    });
                    Err(e)
                }
            }
        }
        .boxed()
        .shared()
    }

    // Check cons && pend_cons or create a new pend_cons
    fn get_or_create(
        inner: Share<PromoteEpInner>,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> impl std::future::Future<Output = KitsuneResult<Self>> + 'static + Send {
        async move {
            let inner2 = inner.clone();
            inner
                .share_mut(|i, _| {
                    if let Some(con_item) = i.cons.get(&remote) {
                        let con_item = con_item.clone();
                        return Ok(async move { Ok(con_item) }.boxed());
                    }
                    if let Some(pend_con_fut) = i.pend_cons.get(&remote) {
                        return Ok(pend_con_fut.clone().boxed());
                    }
                    let con_limit = i.con_limit.clone();
                    let pend_con_fut = Self::inner_con(inner2, con_limit, remote.clone(), timeout);
                    i.pend_cons.insert(remote, pend_con_fut.clone());
                    Ok(pend_con_fut.boxed())
                })?
                .await
        }
    }
}

struct PromoteEpInner {
    con_limit: Arc<Semaphore>,
    logic_hnd: LogicChanHandle<EpEvent>,
    pend_cons: HashMap<TxUrl, Shared<BoxFuture<'static, KitsuneResult<ConItem>>>>,
    cons: HashMap<TxUrl, ConItem>,
    sub_ep: Arc<dyn EndpointAdapt>,
}

struct PromoteEpHnd(Share<PromoteEpInner>, Uniq);

impl PromoteEpHnd {
    pub fn new(
        con_limit: Arc<Semaphore>,
        logic_hnd: LogicChanHandle<EpEvent>,
        sub_ep: Arc<dyn EndpointAdapt>,
    ) -> Self {
        let uniq = sub_ep.uniq();
        Self(
            Share::new(PromoteEpInner {
                con_limit,
                logic_hnd,
                pend_cons: HashMap::new(),
                cons: HashMap::new(),
                sub_ep,
            }),
            uniq,
        )
    }
}

impl AsEpHnd for PromoteEpHnd {
    fn debug(&self) -> serde_json::Value {
        match self.0.share_mut(|i, _| {
            Ok(serde_json::json!({
                "type": "tx2_pool_promote",
                "state": "open",
                "pending_connection_count": i.pend_cons.len(),
                "open_connection_count": i.cons.len(),
                "backend": i.sub_ep.debug(),
            }))
        }) {
            Ok(j) => j,
            Err(_) => serde_json::json!({
                "type": "tx2_pool-promote",
                "state": "closed",
            }),
        }
    }

    fn uniq(&self) -> Uniq {
        self.1
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.share_mut(|i, _| i.sub_ep.local_addr())
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        if let Ok((close_fut, logic_hnd)) = self.0.share_mut(|i, c| {
            *c = true;
            i.con_limit.close();
            Ok((i.sub_ep.close(code, reason), i.logic_hnd.clone()))
        }) {
            async move {
                close_fut.await;
                logic_hnd.close();
            }
            .boxed()
        } else {
            async move {}.boxed()
        }
    }

    fn close_connection(&self, remote: TxUrl, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        if let Ok(Some(con_item)) = self.0.share_mut(|i, _| Ok(i.cons.get(&remote).cloned())) {
            con_item.close(code, reason).boxed()
        } else {
            async move {}.boxed()
        }
    }

    fn get_connection(
        &self,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<ConHnd>> {
        let inner = self.0.clone();
        async move {
            let con_item = ConItem::get_or_create(inner.clone(), remote, timeout).await?;
            let con: ConHnd = Arc::new(con_item);
            Ok(con)
        }
        .boxed()
    }
}

struct PromoteEp {
    hnd: EpHnd,
    logic_chan: LogicChan<EpEvent>,
}

async fn con_recv_logic(
    inner: Share<PromoteEpInner>,
    logic_hnd: LogicChanHandle<EpEvent>,
    _max_cons: usize,
    con_limit: Arc<Semaphore>,
    con_recv: Box<dyn ConRecvAdapt>,
) {
    struct State {
        con_limit: Arc<Semaphore>,
        con_recv: Box<dyn ConRecvAdapt>,
    }

    let state = State {
        con_limit,
        con_recv,
    };

    // Craft this stream carefully.
    // Wait first to acquire a connection permit,
    // Then accept a new pending connection from the con_recv stream.
    // Be sure not to await the pending connection here though.
    // This maintains backpressure at the kernel level,
    // while allowing parallelism / high throughput.
    let pend_stream = futures::stream::unfold(state, move |mut state| async move {
        let permit = match state.con_limit.clone().acquire_owned().await {
            Err(_) => return None,
            Ok(p) => p,
        };
        match state.con_recv.next().await {
            Some(pending) => Some(((permit, pending), state)),
            None => None,
        }
    });

    // Iterate on the pend_stream, handshaking all connections in parallel.
    // This *is* actually bound, by the max connections semaphore.
    let inner = &inner;
    let logic_hnd = &logic_hnd;
    pend_stream
        .for_each_concurrent(None, move |r| async move {
            let (permit, r) = r;
            let (con, in_chan_recv) = match r.await {
                Err(e) => {
                    let _ = logic_hnd.emit(EpEvent::Error(e)).await;
                    return;
                }
                Ok(r) => r,
            };
            let url = match con.remote_addr() {
                Err(e) => {
                    let _ = logic_hnd.emit(EpEvent::Error(e)).await;
                    return;
                }
                Ok(r) => r,
            };
            if let Err(e) = ConItem::reg_con_inner(inner.clone(), permit, con, url.clone(), in_chan_recv, false).await {
                let _ = logic_hnd.emit(EpEvent::Error(e)).await;
            }
        })
        .await;
}

impl PromoteEp {
    pub async fn new(
        max_cons: usize,
        con_limit: Arc<Semaphore>,
        pair: Endpoint,
    ) -> KitsuneResult<Self> {
        let (sub_ep, con_recv) = pair;

        let logic_chan = LogicChan::new(max_cons);
        let logic_hnd = logic_chan.handle().clone();
        let hnd = PromoteEpHnd::new(con_limit.clone(), logic_hnd.clone(), sub_ep);

        let hnd2 = logic_chan.handle().clone();
        hnd2.capture_logic(con_recv_logic(
            hnd.0.clone(),
            logic_hnd,
            max_cons,
            con_limit,
            con_recv,
        ))
        .await?;

        let hnd: EpHnd = Arc::new(hnd);
        Ok(Self { hnd, logic_chan })
    }
}

impl Stream for PromoteEp {
    type Item = EpEvent;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let chan = &mut self.logic_chan;
        futures::pin_mut!(chan);
        Stream::poll_next(chan, cx)
    }
}

impl AsEp for PromoteEp {
    fn handle(&self) -> &EpHnd {
        &self.hnd
    }
}

struct PromoteFactory {
    max_cons: usize,
    backend: BackendFactory,
}

impl AsEpFactory for PromoteFactory {
    fn bind(
        &self,
        bind_spec: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<Ep>> {
        let max_cons = self.max_cons;
        let con_limit = Arc::new(Semaphore::new(max_cons));
        let pair_fut = self.backend.bind(bind_spec, timeout);
        timeout
            .mix(async move {
                let pair = pair_fut.await?;
                let ep = PromoteEp::new(max_cons, con_limit, pair).await?;
                let ep: Ep = Box::new(ep);
                Ok(ep)
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx2_pool_promote() {
        let t = KitsuneTimeout::from_millis(5000);

        const COUNT: usize = 100;
        let (w_send, w_recv) = t_chan(COUNT * 3);

        // we can set the max con count to half...
        // as old connections complete, new ones will be accepted
        let fact = tx2_pool_promote(MemBackendAdapt::new(), COUNT / 2);

        let mut tgt = fact.bind("none:".into(), t).await.unwrap();
        let tgt_hnd = tgt.handle().clone();
        let tgt_addr = tgt_hnd.local_addr().unwrap();

        w_send
            .send(tokio::task::spawn(async move {
                while let Some(evt) = tgt.next().await {
                    match evt {
                        EpEvent::IncomingData(EpIncomingData { con, mut data, .. }) => {
                            assert_eq!(b"hello", data.as_ref());
                            data.clear();
                            data.extend_from_slice(b"world");
                            con.write(0.into(), data, t).await.unwrap();
                        }
                        _ => (),
                    }
                }
            }))
            .await
            .unwrap();

        let mut all_fut = Vec::new();
        for _ in 0..COUNT {
            let ep_fut = fact.bind("none:".into(), t);
            let w_send = w_send.clone();
            let tgt_addr = tgt_addr.clone();
            all_fut.push(async move {
                let mut ep = ep_fut.await.unwrap();
                let ep_hnd = ep.handle().clone();

                let (s_done, r_done) = tokio::sync::oneshot::channel();

                w_send
                    .send(tokio::task::spawn(async move {
                        while let Some(evt) = ep.next().await {
                            match evt {
                                EpEvent::IncomingData(EpIncomingData { data, .. }) => {
                                    assert_eq!(b"world", data.as_ref());
                                    let _ = s_done.send(());
                                    break;
                                }
                                _ => (),
                            }
                        }
                    }))
                    .await
                    .unwrap();

                let mut data = PoolBuf::new();
                data.extend_from_slice(b"hello");
                ep_hnd.write(tgt_addr, 0.into(), data, t).await.unwrap();

                let _ = r_done.await;

                ep_hnd.close(0, "").await;
            });
        }

        futures::future::join_all(all_fut).await;

        tgt_hnd.close(0, "").await;

        w_send.close_channel();

        futures::future::try_join_all(w_recv.collect::<Vec<_>>().await)
            .await
            .unwrap();
    }
}
