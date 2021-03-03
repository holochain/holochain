#![allow(clippy::new_ret_no_self)]
//! Promote a tx2 transport backend to a tx2 transport frontend.

use crate::tx2::tx2_frontend::tx2_frontend_traits::*;
use crate::tx2::tx2_frontend::*;
use crate::tx2::tx_backend::*;
use crate::tx2::util::*;
use crate::tx2::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt};
use futures::stream::{Stream, StreamExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Promote a tx2 transport backend to a tx2 transport frontend.
pub fn promote(backend: BackendFactory, max_cons: usize) -> EpFactory {
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

struct PromoteConInner {
    _permit: OwnedSemaphorePermit,
    con: Arc<dyn ConAdapt>,
    writer_bucket: ResourceBucket<WriteChan>,
}

struct PromoteConHnd(Arc<PromoteConInner>);

impl std::fmt::Debug for PromoteConHnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a = self.0.con.remote_addr().map(|a| a.to_string());
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
    ) -> ConHnd {
        let writer_bucket = ResourceBucket::new();
        let con = Self(Arc::new(PromoteConInner {
            _permit: permit,
            con: raw_con.clone(),
            writer_bucket: writer_bucket.clone(),
        }));
        let con = ConHnd(Arc::new(con));

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
    fn is_closed(&self) -> bool {
        self.0.con.is_closed()
    }

    fn close(&self, code: u32, reason: &str) {
        self.0.con.close(code, reason)
    }

    fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.con.remote_addr()
    }

    fn write(
        &self,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        let inner = self.0.clone();
        async move {
            let mut writer = inner.writer_bucket.acquire(Some(timeout)).await?;
            if let Err(e) = writer.writer.write(msg_id, data, timeout).await {
                let reason = format!("{:?}", e);
                // TODO - standardize codes?
                inner.con.close(500, &reason);
            }
            inner.writer_bucket.release(writer);
            Ok(())
        }
        .boxed()
    }
}

struct PromoteEpInner {
    con_limit: Arc<Semaphore>,
    logic_hnd: LogicChanHandle<EpEvent>,
    ep: Arc<dyn EndpointAdapt>,
}

struct PromoteEpHnd(Arc<PromoteEpInner>);

impl PromoteEpHnd {
    pub fn new(
        con_limit: Arc<Semaphore>,
        logic_hnd: LogicChanHandle<EpEvent>,
        ep: Arc<dyn EndpointAdapt>,
    ) -> Self {
        Self(Arc::new(PromoteEpInner {
            con_limit,
            logic_hnd,
            ep,
        }))
    }
}

impl AsEpHnd for PromoteEpHnd {
    fn is_closed(&self) -> bool {
        self.0.ep.is_closed()
    }

    fn close(&self, code: u32, reason: &str) {
        self.0.logic_hnd.close();
        self.0.ep.close(code, reason);
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.ep.local_addr()
    }

    fn connect(
        &self,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<ConHnd>> {
        let inner = self.0.clone();
        async move {
            let permit = inner.con_limit.clone().acquire_owned().await;
            let (con, in_chan_recv) = inner.ep.connect(remote, timeout).await?;

            let con = PromoteConHnd::new(inner.logic_hnd.clone(), permit, con, in_chan_recv);
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
            let con = PromoteConHnd::new(logic_hnd.clone(), permit, con, in_chan_recv);
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

        logic_chan
            .handle()
            .capture_logic(con_recv_logic(logic_hnd, max_cons, con_limit, con_recv))
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

        let f = promote(MemBackendAdapt::new(), 32);

        let (rs, rr) = tokio::sync::oneshot::channel::<()>();
        let mut rs = Some(rs);

        let mut e1 = f.bind("none:".into(), t).await.unwrap();
        let e1_hnd = e1.handle().clone();
        let rt1 = tokio::task::spawn(async move {
            while let Some(evt) = e1.next().await {
                println!("E1 GOT: {:?}", evt);
                match evt {
                    EpEvent::IncomingData(_, _, data) => {
                        assert_eq!(b"world", data.as_ref());
                        rs.take().unwrap().send(()).unwrap();
                    }
                    _ => (),
                }
            }
        });

        let mut e2 = f.bind("none:".into(), t).await.unwrap();
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

        rr.await.unwrap();

        e1_hnd.close(0, "");
        e2_hnd.close(0, "");

        rt1.await.unwrap();
        rt2.await.unwrap();
    }
}
