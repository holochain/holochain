#![allow(clippy::new_ret_no_self)]
//! Promote a tx2 transport backend to a tx2 transport frontend.

use crate::tx2::tx2_backend::*;
use crate::tx2::tx2_frontend2::tx2_frontend_traits::*;
use crate::tx2::tx2_frontend2::*;
use crate::tx2::tx2_utils::*;
use crate::tx2::*;
use crate::*;
use ghost_actor::dependencies::tracing;
use futures::future::{BoxFuture, FutureExt, Shared};
use futures::stream::{Stream/*, StreamExt*/};
use std::collections::HashMap;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Promote a tx2 transport backend to a tx2 transport frontend.
pub fn tx2_promote(backend: BackendFactory, max_cons: usize) -> EpFactory {
    EpFactory(Arc::new(PromoteFactory { max_cons, backend }))
}

// -- private -- //
//
/// Limit the number of active channels allowed per connection.
const CHANNELS_PER_CONNECTION: usize = 3;

struct WriteChan {
    _permit: OwnedSemaphorePermit,
    writer: OutChan,
}

struct ConItem {
    permit: Option<OwnedSemaphorePermit>,
    con: Arc<dyn ConAdapt>,
    url: TxUrl,
    writer_bucket: ResourceBucket<WriteChan>,
}

impl Drop for ConItem {
    fn drop(&mut self) {
        // keep the permit to limit the max drop task count
        let permit = self.permit.take();

        // we could also have a single drop manager task
        // and forward the cons to be managed, but this is a
        // simple first cut.

        // TODO - standardize codes?
        let close_fut = self.con.close(500, "connection dropped");
        tokio::task::spawn(async move {
            // keeping the permit limits the max drop task count
            let _permit = permit;

            close_fut.await;
        });
    }
}

struct PromoteEpInner {
    con_limit: Arc<Semaphore>,
    pend_cons: HashMap<TxUrl, Shared<BoxFuture<'static, KitsuneResult<(Uniq, Share<ConItem>)>>>>,
    cons: HashMap<TxUrl, (Uniq, Share<ConItem>)>,
    sub_ep: Arc<dyn EndpointAdapt>,
}

// The raw fallible inner future
// `inner_connect` must catch errors to delete the entry from pend_cons
fn inner_connect_inner(
    inner: Share<PromoteEpInner>,
    con_limit: Arc<Semaphore>,
    remote: TxUrl,
    timeout: KitsuneTimeout,
) -> impl std::future::Future<Output = KitsuneResult<(Uniq, Share<ConItem>)>> {
    timeout.mix(async move {
        let permit = con_limit.acquire_owned().await.map_err(KitsuneError::other)?;

        let mut next_wait_ms = 200;

        let try_connect = || async {
            inner.share_mut(|i, _| {
                Ok(i.sub_ep.connect(remote.clone(), timeout))
            })?.await
        };

        loop {
            match try_connect().await {
                Err(e) => tracing::warn!("connect error: {:?}", e),
                Ok((con, _in_chan_recv)) => {
                    let uniq = con.uniq();
                    let con_item = Share::new(ConItem {
                        permit: Some(permit),
                        con,
                        url: remote.clone(),
                        writer_bucket: ResourceBucket::new(),
                    });

                    return Ok((uniq, con_item));
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
fn inner_connect(
    inner: Share<PromoteEpInner>,
    con_limit: Arc<Semaphore>,
    remote: TxUrl,
    timeout: KitsuneTimeout,
) -> Shared<BoxFuture<'static, KitsuneResult<(Uniq, Share<ConItem>)>>> {
    async move {
        match inner_connect_inner(inner.clone(), con_limit, remote.clone(), timeout).await {
            Ok(con_item) => {
                // move us to the full cons list
                inner.share_mut(|i, _| {
                    i.pend_cons.remove(&remote);
                    i.cons.insert(remote, con_item.clone());
                    Ok(())
                })?;
                Ok(con_item)
            }
            Err(e) => {
                // remove the pending entry
                let _ = inner.share_mut(|i, _| {
                    i.pend_cons.remove(&remote);
                    Ok(())
                });
                Err(e)
            }
        }
    }.boxed().shared()
}

// Check cons && pend_cons or create a new pend_cons
fn get_or_create_connection(
    inner: Share<PromoteEpInner>,
    remote: TxUrl,
    timeout: KitsuneTimeout,
) -> impl std::future::Future<Output = KitsuneResult<(Uniq, Share<ConItem>)>> {
    async move {
        let inner2 = inner.clone();
        inner.share_mut(|i, _| {
            if let Some(con_item) = i.cons.get(&remote) {
                let con_item = con_item.clone();
                return Ok(async move { Ok(con_item) }.boxed());
            }
            if let Some(pend_con_fut) = i.pend_cons.get(&remote) {
                return Ok(pend_con_fut.clone().boxed());
            }
            let con_limit = i.con_limit.clone();
            let pend_con_fut = inner_connect(inner2, con_limit, remote.clone(), timeout);
            i.pend_cons.insert(remote, pend_con_fut.clone());
            Ok(pend_con_fut.boxed())
        })?.await
    }
}

struct PromoteEpHnd(Share<PromoteEpInner>, Uniq);

impl PromoteEpHnd {
    pub fn new(
        con_limit: Arc<Semaphore>,
        _logic_hnd: LogicChanHandle<EpEvent>,
        sub_ep: Arc<dyn EndpointAdapt>,
    ) -> Self {
        let uniq = sub_ep.uniq();
        Self(
            Share::new(PromoteEpInner {
                con_limit,
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
        unimplemented!()
    }

    fn uniq(&self) -> Uniq {
        unimplemented!()
    }

    fn is_closed(&self) -> bool {
        unimplemented!()
    }

    fn close(&self, _code: u32, _reason: &str) -> BoxFuture<'static, ()> {
        unimplemented!()
    }

    fn close_connection(
        &self,
        _remote: TxUrl,
        _code: u32,
        _reason: &str,
    ) -> BoxFuture<'static, ()> {
        unimplemented!()
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        unimplemented!()
    }

    fn write(
        &self,
        remote: TxUrl,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        let inner = self.0.clone();
        async move {
            let con_item = get_or_create_connection(inner.clone(), remote.clone(), timeout).await?;
            let mut writer = con_item.1.share_mut(|i, _| {
                Ok(i.writer_bucket.acquire(Some(timeout)))
            })?.await?;
            if let Err(e) = writer.writer.write(msg_id, data, timeout).await {
                let _ = inner.share_mut(|i, _| {
                    let uniq = match i.cons.get(&remote) {
                        None => return Ok(()),
                        Some(ci) => ci.0.clone(),
                    };
                    if uniq == con_item.0 {
                        i.cons.remove(&remote);
                    }
                    Ok(())
                });
                let reason = format!("{:?}", e);
                if let Ok(close_fut) = con_item.1.share_mut(|i, c| {
                    *c = true;
                    // TODO - standardize codes?
                    Ok(i.con.close(500, &reason))
                }) {
                    close_fut.await;
                }
                return Err(e);
            }
            con_item.1.share_mut(move |i, _| {
                i.writer_bucket.release(writer);
                Ok(())
            })?;
            Ok(())
        }.boxed()
    }
}

struct PromoteEp {
    hnd: EpHnd,
    logic_chan: LogicChan<EpEvent>,
}

impl PromoteEp {
    pub async fn new(
        max_cons: usize,
        con_limit: Arc<Semaphore>,
        pair: Endpoint,
    ) -> KitsuneResult<Self> {
        let (sub_ep, _con_recv) = pair;

        let logic_chan = LogicChan::new(max_cons);
        let logic_hnd = logic_chan.handle().clone();
        let hnd = PromoteEpHnd::new(con_limit.clone(), logic_hnd.clone(), sub_ep);

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
        timeout.mix(async move {
            let pair = pair_fut.await?;
            let ep = PromoteEp::new(max_cons, con_limit, pair).await?;
            let ep = Ep(Box::new(ep));
            Ok(ep)
        })
        .boxed()
    }
}
