#![allow(clippy::new_ret_no_self)]
//! Types, traits, and an implementation for applying pooling to a tx backend.

use crate::tx2::tx_backend::*;
use crate::tx2::util::*;
use crate::tx2::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt};
use futures::stream::StreamExt;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

///
#[derive(Debug)]
pub enum TxPoolEvent {
    ///
    ReceiveData(TxUrl, MsgId, PoolBuf),

    ///
    ConnectionClosed(TxUrl, u32, String),
}

// -- concrete -- //

#[allow(dead_code)]
struct ConWrite(OutChan);

impl ConWrite {
    pub fn write(
        &mut self,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'_, KitsuneResult<()>> {
        self.0.write(msg_id, data, timeout)
    }
}

#[allow(dead_code)]
struct ConInner {
    _permit: OwnedSemaphorePermit,
    con: Arc<dyn ConAdapt>,
    write_bucket: ResourceBucket<ConWrite>,
}

#[derive(Clone)]
struct ConRef(Share<ConInner>);

async fn con_inner_logic(
    inner: Share<ConInner>,
    con: Arc<dyn ConAdapt>,
    in_chan_recv: Box<dyn InChanRecvAdapt>,
    timeout: KitsuneTimeout,
) -> KitsuneResult<()> {
    // for now, just always make 3 out chans to start
    let (out1, out2, out3) = futures::future::try_join3(
        con.out_chan(timeout),
        con.out_chan(timeout),
        con.out_chan(timeout),
    )
    .await?;

    // release the out chans to our write resource bucket
    inner.share_mut(move |i, _| {
        i.write_bucket.release(ConWrite(out1));
        i.write_bucket.release(ConWrite(out2));
        i.write_bucket.release(ConWrite(out3));
        Ok(())
    })?;

    drop(inner);
    drop(con);

    // process exactly 3 concurrent incoming channels
    in_chan_recv
        .for_each_concurrent(3, move |in_chan| async move {
            let mut in_chan = match in_chan.await {
                // TODO - FIXME
                Err(e) => panic!("{:?}", e),
                Ok(c) => c,
            };
            while let Ok((_msg_id, _buf)) =
                in_chan.read(KitsuneTimeout::from_millis(1000 * 30)).await
            {
                println!("GOT INCOMING DATA!");
            }
        })
        .await;

    Ok(())
}

impl ConRef {
    pub fn new(
        permit: OwnedSemaphorePermit,
        con: Arc<dyn ConAdapt>,
        in_chan_recv: Box<dyn InChanRecvAdapt>,
        timeout: KitsuneTimeout,
    ) -> Self {
        let inner = Share::new(ConInner {
            _permit: permit,
            con: con.clone(),
            write_bucket: ResourceBucket::new(),
        });

        // This is a calculated task spawn.
        // We could let this future be polled in the top-level endpoint
        // logic channel, but then our system would be single threaded.
        // Instead, we spawn one task per connection, gathering
        // the incoming channel data to be processed.
        let inner2 = inner.clone();
        tokio::task::spawn(async move {
            if let Err(e) = con_inner_logic(inner2, con.clone(), in_chan_recv, timeout).await {
                println!("CONNECTION ERROR: {:?}", e);
                con.close().await;
                // TODO - FIXME - also clean up the connection in the endpoint!!
            }
        });

        Self(inner)
    }

    pub fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.share_mut(|i, _| i.con.remote_addr())
    }

    pub fn write(
        &self,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        let inner = self.0.clone();
        async move {
            let mut con_write = inner
                .share_mut(|i, _| Ok(i.write_bucket.acquire(Some(timeout))))?
                .await?;
            // TODO - FIXME - on error here, we need to close the connection
            con_write.write(msg_id, data, timeout).await?;
            inner.share_mut(move |i, _| {
                i.write_bucket.release(con_write);
                Ok(())
            })?;
            Ok(())
        }
        .boxed()
    }
}

#[allow(dead_code)]
struct TxPoolEpInner {
    max_cons: usize,
    con_limit: Arc<Semaphore>,
    ep: Arc<dyn EndpointAdapt>,
    cons: AsyncMap<TxUrl, ConRef>,
    logic_handle: LogicChanHandle<TxPoolEvent>,
}

///
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TxPoolEpHandle(Share<TxPoolEpInner>);

impl TxPoolEpHandle {
    fn insert_con(&self, con_ref: ConRef) -> BoxFuture<'static, KitsuneResult<()>> {
        let inner = self.0.clone();
        async move {
            let remote_addr = con_ref.remote_addr()?;
            inner.share_mut(move |i, _| i.cons.insert(remote_addr, con_ref))
        }
        .boxed()
    }

    fn get_con(
        &self,
        url: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<ConRef>> {
        let inner = self.0.clone();

        let inner2 = inner.clone();
        let url2 = url.clone();
        let get_logic = move || async move {
            let (limit, ep) =
                inner2.share_mut(move |i, _| Ok((i.con_limit.clone(), i.ep.clone())))?;
            let permit = limit.acquire_owned().await;
            let (con, in_chan_recv) = ep.connect(url2, timeout).await?;
            Ok(ConRef::new(permit, con, in_chan_recv, timeout))
        };

        timeout
            .mix(async move {
                let fut = inner.share_mut(move |i, _| Ok(i.cons.get(url, get_logic)))?;
                fut.await
            })
            .boxed()
    }

    ///
    pub fn local_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.share_mut(|i, _| i.ep.local_addr())
    }

    ///
    pub fn write(
        &self,
        url: TxUrl,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        let con_fut = self.get_con(url, timeout);
        async move {
            con_fut.await?.write(msg_id, data, timeout).await?;
            Ok(())
        }
        .boxed()
    }

    ///
    pub fn close(&self) -> BoxFuture<'static, ()> {
        match self.0.share_mut(|i, c| {
            *c = true;
            Ok(i.ep.close())
        }) {
            Err(_) => async move {}.boxed(),
            Ok(fut) => fut,
        }
    }
}

///
pub struct TxPoolEp {
    handle: TxPoolEpHandle,
    logic_chan: LogicChan<TxPoolEvent>,
}

async fn con_recv_logic(
    handle: TxPoolEpHandle,
    con_recv: Box<dyn ConRecvAdapt>,
    max_cons: usize,
    con_limit: Arc<Semaphore>,
) {
    let con_limit = &con_limit;
    let con_recv = con_recv.map(move |fut| async move {
        let permit = con_limit.clone().acquire_owned().await;
        let (con, in_chan_recv) = match fut.await {
            Err(e) => return Err(e),
            Ok(r) => r,
        };
        Ok((permit, con, in_chan_recv))
    });
    let handle = &handle;
    con_recv
        .for_each_concurrent(max_cons, move |fut| async move {
            let (permit, con, in_chan_recv) = match fut.await {
                // TODO - FIXME
                Err(e) => panic!("{:?}", e),
                Ok(r) => r,
            };
            let t = KitsuneTimeout::from_millis(1000 * 30);
            let con_ref = ConRef::new(permit, con, in_chan_recv, t);
            if let Err(e) = handle.insert_con(con_ref).await {
                // TODO - FIXME
                panic!("{:?}", e);
            }
        })
        .await;
}

impl TxPoolEp {
    ///
    async fn new(
        ep: Arc<dyn EndpointAdapt>,
        con_recv: Box<dyn ConRecvAdapt>,
        max_cons: usize,
        con_limit: Arc<Semaphore>,
    ) -> KitsuneResult<Self> {
        let logic_chan = LogicChan::new(32);
        let logic_handle = logic_chan.handle().clone();

        let handle = TxPoolEpHandle(Share::new(TxPoolEpInner {
            max_cons,
            con_limit: con_limit.clone(),
            ep,
            cons: AsyncMap::new(),
            logic_handle,
        }));

        logic_chan
            .handle()
            .capture_logic(con_recv_logic(
                handle.clone(),
                con_recv,
                max_cons,
                con_limit,
            ))
            .await?;

        Ok(Self { handle, logic_chan })
    }

    ///
    pub fn handle(&self) -> &TxPoolEpHandle {
        &self.handle
    }
}

impl futures::stream::Stream for TxPoolEp {
    type Item = TxPoolEvent;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let inner = &mut self.logic_chan;
        tokio::pin!(inner);
        futures::stream::Stream::poll_next(inner, cx)
    }
}

///
#[allow(dead_code)]
pub struct TxPoolFactoryWrapper {
    sub_fact: BackendFactory,
    max_cons: usize,
    con_limit: Arc<Semaphore>,
}

impl TxPoolFactoryWrapper {
    ///
    pub fn new(sub_fact: BackendFactory, max_cons: usize) -> Self {
        let con_limit = Arc::new(Semaphore::new(max_cons));
        Self {
            sub_fact,
            max_cons,
            con_limit,
        }
    }

    ///
    pub fn bind(
        &self,
        url: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<TxPoolEp>> {
        let ep_fut = self.sub_fact.bind(url, timeout);
        let max_cons = self.max_cons;
        let con_limit = self.con_limit.clone();
        async move {
            let (ep, con_recv) = ep_fut.await?;

            TxPoolEp::new(ep, con_recv, max_cons, con_limit).await
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_tx_pool() {
        let t = KitsuneTimeout::from_millis(5000);

        let fact = TxPoolFactoryWrapper::new(MemBackendAdapt::new(), 32);

        let pool1 = fact.bind("none:".into(), t).await.unwrap();
        let mut pool2 = fact.bind("none:".into(), t).await.unwrap();
        let pool2handle = pool2.handle().clone();

        let addr2 = pool2handle.local_addr().unwrap();
        println!("got addr2: {}", addr2);

        let rt = tokio::task::spawn(async move {
            while let Some(evt) = pool2.next().await {
                println!("GOT EVT: {:?}", evt);
            }
        });

        let mut buf = PoolBuf::new();
        buf.extend_from_slice(b"hello");
        pool1.handle().write(addr2, 0.into(), buf, t).await.unwrap();

        pool1.handle().close().await;
        pool2handle.close().await;

        rt.await.unwrap();
    }
}

/*
use crate::tx2::tx_backend::*;
use crate::tx2::util::*;
use crate::tx2::*;
use crate::*;

use futures::future::{BoxFuture, FutureExt};
use futures::stream::{Stream, StreamExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use std::collections::HashMap;

///
pub enum TxPoolEvent {
    ///
    ReceiveData(TxUrl, MsgId, PoolBuf),

    ///
    ConnectionClosed(TxUrl, u32, String),
}

///
pub trait TxPoolEventRecv: 'static + Send + Unpin + Stream<Item = TxPoolEvent> {
}

///
pub trait TxPoolAdapt: 'static + Send + Sync + Unpin {
    ///
    fn local_addr(&self) -> KitsuneResult<TxUrl>;

    ///
    fn write(
        &self,
        url: TxUrl,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>>;

    ///
    fn close(&self) -> BoxFuture<'static, ()>;
}

///
pub type TxPoolPair = (Arc<dyn TxPoolAdapt>, Box<dyn TxPoolEventRecv>);

///
pub type TxPoolPairFut = BoxFuture<'static, KitsuneResult<TxPoolPair>>;

///
pub trait TxPoolBackendAdapt: 'static + Send + Sync + Unpin {
    ///
    fn bind(&self, url: TxUrl, timeout: KitsuneTimeout) -> TxPoolPairFut;
}

///
pub type TxPoolFactory = Arc<dyn TxPoolBackendAdapt>;

// -- basic implementation of TxPool over backend traits -- //

struct Share<T>(Arc<parking_lot::RwLock<Option<T>>>);

impl<T> Clone for Share<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Share<T> {
    pub fn new(t: T) -> Self {
        Self(Arc::new(parking_lot::RwLock::new(Some(t))))
    }

    pub fn share_ref<R, F>(&self, f: F) -> KitsuneResult<R>
    where
        F: FnOnce(&T) -> KitsuneResult<R>,
    {
        let t = self.0.read();
        if t.is_none() {
            return Err(KitsuneError::Closed);
        }
        f(t.as_ref().unwrap())
    }

    pub fn share_mut<R, F>(&self, f: F) -> KitsuneResult<R>
    where
        F: FnOnce(&mut T, &mut bool) -> KitsuneResult<R>,
    {
        let mut t = self.0.write();
        if t.is_none() {
            return Err(KitsuneError::Closed);
        }
        let mut close = false;
        let r = f(t.as_mut().unwrap(), &mut close);
        if close {
            *t = None;
        }
        r
    }
}

struct EvtRecvWrap {
    actor: Actor<TxPoolEvent>,
    pending: Vec<TxPoolEvent>,
}

impl EvtRecvWrap {
    pub fn new(actor: Actor<TxPoolEvent>) -> Box<dyn TxPoolEventRecv> {
        Box::new(Self {
            actor,
            pending: Vec::new(),
        })
    }
}

impl Stream for EvtRecvWrap {
    type Item = TxPoolEvent;


}

impl TxPoolEventRecv for EvtRecvWrap {}
    fn next(&mut self) -> BoxFuture<'_, KitsuneResult<TxPoolEvent>> {
        async move {
            if self.pending.is_empty() {
                let mut items = match self.actor.next().await {
                    None => return Err(KitsuneError::Closed),
                    Some(items) => items,
                };
                self.pending.append(&mut items);
            }
            Ok(self.pending.remove(0))
        }
        .boxed()
    }
}

#[derive(Debug)]
struct ConRef {
    last_msg: std::time::Instant,
}

type ConRefFut = futures::future::Shared<BoxFuture<'static, Arc<KitsuneResult<ConRef>>>>;

struct PoolWrapInner {
    ep: Arc<dyn EndpointAdapt>,
    cons: HashMap<TxUrl, ConRefFut>,
}

impl PoolWrapInner {
    pub fn new(ep: Arc<dyn EndpointAdapt>) -> Self {
        Self {
            ep,
            cons: HashMap::new(),
        }
    }
}

struct PoolWrap(Share<PoolWrapInner>);

impl PoolWrap {
    pub fn new(ep: Arc<dyn EndpointAdapt>) -> Arc<dyn TxPoolAdapt> {
        Arc::new(Self(Share::new(PoolWrapInner::new(ep))))
    }

    fn get_con(&self, url: TxUrl, timeout: KitsuneTimeout) -> KitsuneResult<ConRefFut> {
        let inner = self.0.clone();
        self.0.share_mut(move |i, _| {
            let ep = i.ep.clone();
            Ok(i.cons
                .entry(url.clone())
                .or_insert_with(|| {
                    let fut = ep.connect(url.clone(), timeout);
                    async move {
                        let (_con, _in_chan_recv) = match fut.await {
                            Err(e) => {
                                inner
                                    .share_mut(move |i, _| {
                                        i.cons.remove(&url);
                                        Ok(())
                                    })
                                    .unwrap();
                                return Arc::new(Err(e));
                            }
                            Ok(c) => c,
                        };
                        Arc::new(Ok(ConRef {
                            last_msg: std::time::Instant::now(),
                        }))
                    }
                    .boxed()
                    .shared()
                })
                .clone())
        })
    }
}

impl TxPoolAdapt for PoolWrap {
    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.share_ref(|i| i.ep.local_addr())
    }

    fn write(
        &self,
        url: TxUrl,
        _msg_id: MsgId,
        _data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        let con_fut = self.get_con(url, timeout);
        async move {
            let con_ref = con_fut?.await;
            println!("{:?}", con_ref);

            unimplemented!()
        }
        .boxed()
    }

    fn close(&self) -> BoxFuture<'static, ()> {
        match self.0.share_mut(|i, c| {
            *c = true;
            Ok(i.ep.close())
        }) {
            Err(_) => async move {}.boxed(),
            Ok(fut) => fut,
        }
    }
}

/// Wrap a basic BackendFactory into a TxPoolFactory.
pub struct TxPoolFactoryWrapper {
    sub_factory: BackendFactory,
    max_connections: Arc<Semaphore>,
}

impl TxPoolFactoryWrapper {
    /// Wrap a basic BackendFactory into a TxPoolFactory.
    pub fn new(sub_factory: BackendFactory, max_connections: usize) -> TxPoolFactory {
        let out: TxPoolFactory = Arc::new(Self {
            sub_factory,
            max_connections: Arc::new(Semaphore::new(max_connections)),
        });
        out
    }
}

async fn recv_con_logic(con_recv: Box<dyn ConRecvAdapt>, max_connections: Arc<Semaphore>) {
    type P = (
        OwnedSemaphorePermit,
        Arc<dyn ConAdapt>,
        Box<dyn InChanRecvAdapt>,
    );
    type RP = BoxFuture<'static, KitsuneResult<P>>;
    type SP = futures::stream::BoxStream<'static, RP>;
    let con_recv: SP = futures::stream::unfold(con_recv, move |mut con_recv| {
        let max_connections = max_connections.clone();
        async move {
            let permit = max_connections.acquire_owned().await;
            match con_recv.next().await {
                Err(_) => None,
                Ok(fut) => Some((
                    async move {
                        let (con, chan_recv) = fut.await?;
                        Ok((permit, con, chan_recv))
                    }
                    .boxed(),
                    con_recv,
                )),
            }
        }
    })
    .boxed();
    con_recv
        .for_each_concurrent(None, move |fut| async move {
            let (_permit, con, _chan_recv) = match fut.await {
                // TODO - FIXME
                Err(e) => panic!("{:?}", e),
                Ok(r) => r,
            };
            println!("RECV CON: rem: {}", con.remote_addr().unwrap());
            unimplemented!()
        })
        .await;
    println!("CON RECV LOOP END");
}

impl TxPoolBackendAdapt for TxPoolFactoryWrapper {
    fn bind(&self, url: TxUrl, timeout: KitsuneTimeout) -> TxPoolPairFut {
        let ep_fut = self.sub_factory.bind(url, timeout);
        let max_connections = self.max_connections.clone();
        async move {
            let (ep, con_recv) = ep_fut.await?;

            let actor = Actor::new(32);
            let pool = PoolWrap::new(ep);

            actor
                .handle()
                .capture_logic(recv_con_logic(con_recv, max_connections.clone()))
                .await;

            let recv = EvtRecvWrap::new(actor);

            Ok((pool, recv))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_tx_pool() {
        let t = KitsuneTimeout::from_millis(5000);

        let pfact = TxPoolFactoryWrapper::new(MemBackendAdapt::new(), 32);

        let (p1, _recv1) = pfact.bind("none:".into(), t).await.unwrap();
        let (p2, mut recv2) = pfact.bind("none:".into(), t).await.unwrap();

        let rt = tokio::task::spawn(async move {
            while let Ok(ev) = recv2.next().await {
                match ev {
                    TxPoolEvent::ReceiveData(url, _msg_id, _buf) => {
                        println!("GOT DATA from {}", url);
                    }
                    TxPoolEvent::ConnectionClosed(url, _code, _reason) => {
                        println!("CONNECTION CLOSED: {}", url);
                    }
                }
            }
        });

        let addr2 = p2.local_addr().unwrap();
        println!("binding2: {}", addr2);

        let mut buf = PoolBuf::new();
        buf.extend_from_slice(b"hello");
        p1.write(addr2, 0.into(), buf, t).await.unwrap();

        p1.close().await;
        p2.close().await;

        rt.await.unwrap();
    }
}
*/
