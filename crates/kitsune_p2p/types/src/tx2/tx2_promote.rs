#![allow(clippy::new_ret_no_self)]
//! Promote a tx2 transport backend to a tx2 transport frontend.

use crate::tx2::tx2_frontend::tx2_frontend_traits::*;
use crate::tx2::tx2_frontend::*;
use crate::tx2::tx2_backend::*;
use crate::tx2::tx2_utils::*;
use crate::tx2::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt};
use futures::stream::{Stream, StreamExt};
use std::collections::HashSet;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Promote a tx2 transport backend to a tx2 transport frontend.
pub fn tx2_promote(backend: BackendFactory, max_cons: usize) -> EpFactory {
    let con_limit = Arc::new(Semaphore::new(max_cons));
    EpFactory(Arc::new(PromoteFactory {
        max_cons,
        con_limit,
        backend,
    }))
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

struct PromoteConHnd(Share<PromoteConInner>, Uniq);

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
    raw_con: Arc<dyn ConAdapt>,
    con: ConHnd,
    writer_bucket: ResourceBucket<WriteChan>,
    logic_hnd: LogicChanHandle<EpEvent>,
    in_chan_recv: Box<dyn InChanRecvAdapt>,
) {
    let con = &con;
    let logic_hnd = &logic_hnd;

    let recv_fut = in_chan_recv
        .for_each_concurrent(CHANNELS_PER_CONNECTION, move |chan| async move {
            let mut chan = match chan.await {
                Ok(c) => c,
                // TODO - FIXME
                Err(e) => panic!("{:?}", e),
            };
            loop {
                let r = chan
                    .read(KitsuneTimeout::from_millis(MAX_READ_TIMEOUT))
                    .await;
                let (msg_id, data) = match r {
                    Ok(r) => r,
                    // TODO - FIXME
                    Err(e) => panic!("{:?}", e),
                };

                if logic_hnd
                    .emit(EpEvent::IncomingData(con.clone(), msg_id, data))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        })
        .boxed();

    let write_fut = async move {
        let limit = Arc::new(Semaphore::new(CHANNELS_PER_CONNECTION));
        loop {
            // TODO - FIXME - this loop may leak
            // would be nice if we had tokio 1.0 kill-able Semaphore.
            let permit = limit.clone().acquire_owned().await;
            let writer = match raw_con
                .out_chan(KitsuneTimeout::from_millis(MAX_READ_TIMEOUT))
                .await
            {
                // TODO - FIXME
                Err(e) => panic!("{:?}", e),
                Ok(c) => c,
            };
            writer_bucket.release(WriteChan {
                _permit: permit,
                writer,
            });
        }
    };

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
        let standin = ConHnd(Arc::new(Self(Share::new_closed(), Uniq::default())));

        let uniq = raw_con.uniq();
        let writer_bucket = ResourceBucket::new();
        let con = Self(
            Share::new(PromoteConInner {
                _permit: permit,
                hnd: standin,
                con: raw_con.clone(),
                close_con: Some(close_con),
                writer_bucket: writer_bucket.clone(),
            }),
            uniq,
        );

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
    match this.share_mut(move |i, c| {
        *c = true;
        let f1 = if let Some(close_con) = i.close_con.take() {
            close_con(i.hnd.clone(), code, reason)
        } else {
            async move {}.boxed()
        };
        let f2 = i.con.close(code, reason);
        Ok(async move {
            futures::future::join(f1, f2).await;
        }
        .boxed())
    }) {
        Ok(fut) => fut,
        Err(_) => async move {}.boxed(),
    }
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
            logic_hnd
                .emit(EpEvent::ConnectionClosed(con, code, reason.to_string()))
                .map(|_| ())
                .boxed()
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
            let permit = limit.acquire_owned().await;
            let (con, in_chan_recv) = ep.connect(remote, timeout).await?;

            let con = reg_con_hnd_inner(&inner, permit, con, in_chan_recv)?;
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
        let permit = state.con_limit.clone().acquire_owned().await;
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
            let con = match reg_con_hnd_inner(&inner, permit, con, in_chan_recv) {
                Err(e) => {
                    let _ = logic_hnd.emit(EpEvent::Error(e));
                    return;
                }
                Ok(r) => r,
            };
            let _ = logic_hnd.emit(EpEvent::IncomingConnection(con));
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

        let logic_chan = LogicChan::new(32);
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
    con_limit: Arc<Semaphore>,
    backend: BackendFactory,
}

impl AsEpFactory for PromoteFactory {
    fn bind(
        &self,
        bind_spec: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<Ep>> {
        let max_cons = self.max_cons;
        let con_limit = self.con_limit.clone();
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

    #[tokio::test(threaded_scheduler)]
    async fn test_tx2_backend_frontend_promote() {
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
                    EpEvent::IncomingData(_, _, data) => {
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
                    EpEvent::IncomingData(con, _, mut data) => {
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

        e1_hnd.close(0, "").await;
        e2_hnd.close(0, "").await;

        rt1.await.unwrap();
        rt2.await.unwrap();
    }
}
