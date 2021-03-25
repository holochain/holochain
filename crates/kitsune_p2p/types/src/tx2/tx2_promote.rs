#![allow(clippy::new_ret_no_self)]
//! Promote a tx2 transport backend to a tx2 transport frontend.

use crate::tx2::tx2_backend::*;
use crate::tx2::tx2_frontend::tx2_frontend_traits::*;
use crate::tx2::tx2_frontend::*;
use crate::tx2::tx2_utils::*;
use crate::tx2::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt};
use futures::stream::{Stream, StreamExt};
use std::collections::HashSet;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Promote a tx2 transport backend to a tx2 transport frontend.
pub fn tx2_promote(backend: BackendFactory, max_cons: usize) -> EpFactory {
    EpFactory(Arc::new(PromoteFactory { max_cons, backend }))
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

type CloseCon = Box<dyn FnOnce(ConHnd, u32, &str) -> BoxFuture<'static, ()> + 'static + Send>;

struct PromoteConInner {
    _permit: OwnedSemaphorePermit,
    hnd: ConHnd,
    con: Arc<dyn ConAdapt>,
    close_con: Option<CloseCon>,
    writer_bucket: ResourceBucket<WriteChan>,
}

struct PromoteConHnd(Arc<Share<PromoteConInner>>, Uniq);

impl std::fmt::Debug for PromoteConHnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a = self
            .0
            .share_mut(|i, _| i.con.remote_addr().map(|a| a.to_string()))
            .unwrap_or_else(|_| "[closed]".to_string());
        f.debug_tuple("ConHnd").field(&a).finish()
    }
}

async fn in_chan_recv_logic(
    inner: Arc<Share<PromoteConInner>>,
    raw_con: Arc<dyn ConAdapt>,
    con: ConHnd,
    writer_bucket: ResourceBucket<WriteChan>,
    logic_hnd: LogicChanHandle<EpEvent>,
    in_chan_recv: Box<dyn InChanRecvAdapt>,
) {
    let url = match con.remote_addr() {
        Err(e) => {
            // unable to determine remote url for connection

            let reason = format!("{:?}", e);

            // TODO - standardize codes?
            close_inner(&inner, 500, &reason).await;

            // don't even start the loops
            return;
        }
        Ok(url) => url,
    };

    let url = &url;
    let con = &con;
    let logic_hnd = &logic_hnd;
    let inner = &inner;

    let recv_fut = in_chan_recv
        .for_each_concurrent(CHANNELS_PER_CONNECTION, move |chan| async move {
            let mut chan = match chan.await {
                Err(e) => {
                    // unable to resolve incoming channel
                    // shut down the connection

                    let reason = format!("{:?}", e);

                    // TODO - standardize codes?
                    close_inner(inner, 500, &reason).await;

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
                        close_inner(inner, 500, &reason).await;

                        // exit the loop
                        return;
                    }
                    Ok(r) => r,
                };

                let data = EpIncomingData {
                    url: url.clone(),
                    con: con.clone(),
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

            let writer = match raw_con
                .out_chan(KitsuneTimeout::from_millis(MAX_READ_TIMEOUT))
                .await
            {
                Err(e) => {
                    // we were not able to create an outgoing channel
                    // clean up the connection.

                    let reason = format!("{:?}", e);

                    // TODO - standardize codes?
                    close_inner(inner, 500, &reason).await;

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

impl PromoteConHnd {
    pub fn new(
        logic_hnd: LogicChanHandle<EpEvent>,
        permit: OwnedSemaphorePermit,
        raw_con: Arc<dyn ConAdapt>,
        in_chan_recv: Box<dyn InChanRecvAdapt>,
        close_con: CloseCon,
    ) -> ConHnd {
        let standin = ConHnd(Arc::new(Self(
            Arc::new(Share::new_closed()),
            Uniq::default(),
        )));

        let uniq = raw_con.uniq();
        let writer_bucket = ResourceBucket::new();
        let inner = Arc::new(Share::new(PromoteConInner {
            _permit: permit,
            hnd: standin,
            con: raw_con.clone(),
            close_con: Some(close_con),
            writer_bucket: writer_bucket.clone(),
        }));
        let con = Self(inner.clone(), uniq);

        let a = Arc::new(con);
        let con = ConHnd(a.clone());
        let hnd = con.clone();
        a.0.share_mut(move |i, _| {
            i.hnd = hnd;
            Ok(())
        })
        .unwrap();

        // This is a calculated task spawn.
        // We could let this future be polled in the top-level endpoint
        // logic channel, but then our system would be single threaded.
        // Instead, we spawn one task per connection, gathering
        // the incoming channel data to be processed.
        tokio::task::spawn(in_chan_recv_logic(
            inner,
            raw_con,
            con.clone(),
            writer_bucket,
            logic_hnd,
            in_chan_recv,
        ));

        con
    }
}

impl AsConHnd for PromoteConHnd {
    fn uniq(&self) -> Uniq {
        self.1
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        close_inner(&self.0, code, reason)
    }

    fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.share_mut(|i, _| i.con.remote_addr())
    }

    fn write(
        &self,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        let inner = self.0.clone();
        async move {
            let mut writer = inner
                .share_mut(|i, _| Ok(i.writer_bucket.acquire(Some(timeout))))?
                .await?;
            if let Err(e) = writer.writer.write(msg_id, data, timeout).await {
                let reason = format!("{:?}", e);
                // TODO - standardize codes?
                close_inner(&inner, 500, &reason).await;
                return Err(e);
            }
            inner.share_mut(|i, _| {
                i.writer_bucket.release(writer);
                Ok(())
            })?;
            Ok(())
        }
        .boxed()
    }
}

fn close_inner(this: &Share<PromoteConInner>, code: u32, reason: &str) -> BoxFuture<'static, ()> {
    let (close_con, hnd, con) = match this.share_mut(move |i, c| {
        *c = true;
        Ok((i.close_con.take(), i.hnd.clone(), i.con.clone()))
    }) {
        Err(_) => return async move {}.boxed(),
        Ok(r) => r,
    };
    let f1 = if let Some(close_con) = close_con {
        close_con(hnd, code, reason)
    } else {
        async move {}.boxed()
    };
    let f2 = con.close(code, reason);
    async move {
        futures::future::join(f1, f2).await;
    }
    .boxed()
}

struct PromoteEpInner {
    con_limit: Arc<Semaphore>,
    logic_hnd: LogicChanHandle<EpEvent>,
    ep: Arc<dyn EndpointAdapt>,
    all_cons: HashSet<ConHnd>,
}

struct PromoteEpHnd(Share<PromoteEpInner>, Uniq);

impl PromoteEpHnd {
    pub fn new(
        con_limit: Arc<Semaphore>,
        logic_hnd: LogicChanHandle<EpEvent>,
        ep: Arc<dyn EndpointAdapt>,
    ) -> Self {
        let uniq = ep.uniq();
        Self(
            Share::new(PromoteEpInner {
                con_limit,
                logic_hnd,
                ep,
                all_cons: HashSet::new(),
            }),
            uniq,
        )
    }
}

fn reg_con_hnd_inner(
    inner: &Share<PromoteEpInner>,
    permit: OwnedSemaphorePermit,
    raw_con: Arc<dyn ConAdapt>,
    url: TxUrl,
    in_chan_recv: Box<dyn InChanRecvAdapt>,
) -> KitsuneResult<ConHnd> {
    let inner2 = inner.clone();
    inner.share_mut(move |i, _| {
        let logic_hnd = i.logic_hnd.clone();
        let con_close: CloseCon = Box::new(move |con, code, reason| {
            let _ = inner2.share_mut(|i, _| {
                i.all_cons.remove(&con);
                Ok(())
            });
            let evt = EpEvent::ConnectionClosed(EpConnectionClosed {
                con,
                url,
                code,
                reason: reason.to_string(),
            });
            logic_hnd.emit(evt).map(|_| ()).boxed()
        });
        let con = PromoteConHnd::new(
            i.logic_hnd.clone(),
            permit,
            raw_con,
            in_chan_recv,
            con_close,
        );
        i.all_cons.insert(con.clone());
        Ok(con)
    })
}

impl AsEpHnd for PromoteEpHnd {
    fn debug(&self) -> serde_json::Value {
        match self.0.share_mut(|i, _| {
            Ok(serde_json::json!({
                "type": "tx2_promote",
                "state": "open",
                "connection_count": i.all_cons.len(),
                "backend": i.ep.debug(),
            }))
        }) {
            Ok(j) => j,
            Err(_) => serde_json::json!({
                "type": "tx2_promote",
                "state": "closed",
            }),
        }
    }

    fn uniq(&self) -> Uniq {
        self.1
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        // we have to be careful with this so we don't deadlock on
        // the connection cleanup, but we actually get closed.
        let reason = reason.to_string();
        let inner = self.0.clone();
        async move {
            let (ep, cons, logic_hnd) = inner.share_mut(|i, c| {
                *c = true;
                i.con_limit.close();
                Ok((
                    i.ep.clone(),
                    i.all_cons.iter().cloned().collect::<Vec<_>>(),
                    i.logic_hnd.clone(),
                ))
            })?;
            ep.close(code, &reason).await;
            futures::future::join_all(cons.into_iter().map(|c| c.close(code, &reason))).await;
            let _ = logic_hnd.emit(EpEvent::EndpointClosed).await;
            logic_hnd.close();
            KitsuneResult::Ok(())
        }
        .map(|_| ())
        .boxed()
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.share_mut(|i, _| i.ep.local_addr())
    }

    fn connect(
        &self,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<ConHnd>> {
        let r = self
            .0
            .share_mut(|i, _| Ok((i.con_limit.clone(), i.ep.clone())));
        let inner = self.0.clone();
        async move {
            let (limit, ep) = r?;
            let permit = limit.acquire_owned().await.map_err(KitsuneError::other)?;
            let (con, in_chan_recv) = ep.connect(remote, timeout).await?;

            let url = con.remote_addr()?;
            let con = reg_con_hnd_inner(&inner, permit, con, url, in_chan_recv)?;
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
                    let _ = logic_hnd.emit(EpEvent::Error(e));
                    return;
                }
                Ok(r) => r,
            };
            let url = match con.remote_addr() {
                Err(e) => {
                    let _ = logic_hnd.emit(EpEvent::Error(e));
                    return;
                }
                Ok(r) => r,
            };
            let con = match reg_con_hnd_inner(&inner, permit, con, url.clone(), in_chan_recv) {
                Err(e) => {
                    let _ = logic_hnd.emit(EpEvent::Error(e));
                    return;
                }
                Ok(r) => r,
            };
            let evt = EpEvent::IncomingConnection(EpIncomingConnection { con, url });
            let _ = logic_hnd.emit(evt);
        })
        .await;
}

impl PromoteEp {
    pub async fn new(
        max_cons: usize,
        con_limit: Arc<Semaphore>,
        pair: Endpoint,
    ) -> KitsuneResult<Self> {
        let (ep, con_recv) = pair;

        let logic_chan = LogicChan::new(max_cons);
        let logic_hnd = logic_chan.handle().clone();
        let hnd = PromoteEpHnd::new(con_limit.clone(), logic_hnd.clone(), ep);

        let hnd2 = logic_chan.handle().clone();

        hnd2.capture_logic(con_recv_logic(
            hnd.0.clone(),
            logic_hnd,
            max_cons,
            con_limit,
            con_recv,
        ))
        .await?;

        let hnd = EpHnd(Arc::new(hnd));
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
        async move {
            let pair = pair_fut.await?;
            let ep = PromoteEp::new(max_cons, con_limit, pair).await?;
            let ep = Ep(Box::new(ep));
            Ok(ep)
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx2_promote_stress() {
        let t = KitsuneTimeout::from_millis(5000);

        const COUNT: usize = 100;
        let (w_send, w_recv) = t_chan(COUNT * 3);

        // we can set the max con count to half...
        // as old connections complete, new ones will be accepted
        let fact = tx2_promote(MemBackendAdapt::new(), COUNT / 2);

        let mut tgt = fact.bind("none:", t).await.unwrap();
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
            let ep_fut = fact.bind("none:", t);
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

                let con = ep_hnd.connect(tgt_addr, t).await.unwrap();
                let mut data = PoolBuf::new();
                data.extend_from_slice(b"hello");
                con.write(0.into(), data, t).await.unwrap();

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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx2_promote() {
        let t = KitsuneTimeout::from_millis(5000);

        let f = tx2_promote(MemBackendAdapt::new(), 32);

        let (t_comp_s, t_comp_r) = tokio::sync::oneshot::channel::<()>();
        let mut t_comp_s = Some(t_comp_s);

        let mut e1 = f.bind("none:", t).await.unwrap();
        let e1_hnd = e1.handle().clone();
        let rt1 = tokio::task::spawn(async move {
            while let Some(evt) = e1.next().await {
                println!("E1 GOT: {:?}", evt);
                match evt {
                    EpEvent::IncomingData(EpIncomingData { data, .. }) => {
                        assert_eq!(b"world", data.as_ref());
                        t_comp_s.take().unwrap().send(()).unwrap();
                    }
                    _ => (),
                }
            }
        });

        let mut e2 = f.bind("none:", t).await.unwrap();
        let e2_hnd = e2.handle().clone();
        let rt2 = tokio::task::spawn(async move {
            while let Some(evt) = e2.next().await {
                println!("E2 GOT: {:?}", evt);
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
        });

        let addr2 = e2_hnd.local_addr().unwrap();
        println!("got addr2: {}", addr2);

        let con = e1_hnd.connect(addr2, t).await.unwrap();

        let mut data = PoolBuf::new();
        data.extend_from_slice(b"hello");
        con.write(0.into(), data, t).await.unwrap();

        t_comp_r.await.unwrap();

        let debug = e1_hnd.debug();
        println!("{}", serde_json::to_string_pretty(&debug).unwrap());

        e1_hnd.close(0, "").await;
        e2_hnd.close(0, "").await;

        rt1.await.unwrap();
        rt2.await.unwrap();
    }
}
