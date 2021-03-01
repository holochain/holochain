#![allow(clippy::new_ret_no_self)]
//! Types, traits, and an implementation for applying pooling to a tx backend.

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
