#![allow(clippy::new_ret_no_self)]
//! Next-gen performance kitsune transport proxy

use crate::*;
use futures::future::BoxFuture;
use futures::stream::{Stream, StreamExt};
use kitsune_p2p_types::tx2::tx2_frontend::tx2_frontend_traits::*;
use kitsune_p2p_types::tx2::tx2_frontend::*;
use kitsune_p2p_types::tx2::tx2_promote::*;
use kitsune_p2p_types::tx2::tx_backend::*;
use kitsune_p2p_types::tx2::util::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::collections::HashMap;

/// Wrap a tx2 backend transport with proxy logic.
pub fn tx2_proxy(sub_tx: BackendFactory, tls_config: TlsConfig, max_cons: usize) -> EpFactory {
    ProxyEpFactory::new(sub_tx, tls_config, max_cons)
}

// -- private -- //

#[allow(dead_code)]
#[derive(Clone)]
enum ConRef {
    Ready(ConHnd),
    Pending(futures::future::Shared<BoxFuture<'static, KitsuneResult<ConHnd>>>),
}

#[allow(dead_code)]
struct ProxyEpInner {
    proxy_cons: HashMap<CertDigest, ConRef>,
    all_cons: HashMap<CertDigest, ConRef>,
}

#[allow(dead_code)]
struct ProxyEpHnd {
    sub_ep_hnd: EpHnd,
    cert_digest: CertDigest,
    inner: Share<ProxyEpInner>,
}

impl ProxyEpHnd {
    pub fn new(sub_ep_hnd: EpHnd, cert_digest: CertDigest) -> Arc<ProxyEpHnd> {
        Arc::new(ProxyEpHnd {
            sub_ep_hnd,
            cert_digest,
            inner: Share::new(ProxyEpInner {
                proxy_cons: HashMap::new(),
                all_cons: HashMap::new(),
            }),
        })
    }
}

const PROXY_HELLO_DIGEST: u8 = 0x20;

async fn ingest_outgoing_con(
    inner: Share<ProxyEpInner>,
    local_digest: CertDigest,
    remote_digest: CertDigest,
    sub_con: ConHnd,
    timeout: KitsuneTimeout,
) -> KitsuneResult<ConHnd> {
    let mut data = PoolBuf::new();
    data.reserve(local_digest.len() + 1);
    data.extend_from_slice(&[PROXY_HELLO_DIGEST]);
    data.extend_from_slice(&local_digest);
    sub_con.write(0.into(), data, timeout).await?;

    let sub_con2 = sub_con.clone();
    inner.share_mut(move |i, _| {
        i.all_cons.insert(remote_digest, ConRef::Ready(sub_con2));
        Ok(())
    })?;
    Ok(sub_con)
}

async fn clear_outgoing_con(
    inner: Share<ProxyEpInner>,
    remote_digest: CertDigest,
    err: KitsuneError,
) -> KitsuneError {
    let _ = inner.share_mut(move |i, _| {
        i.all_cons.remove(&remote_digest);
        Ok(())
    });
    err
}

impl AsEpHnd for ProxyEpHnd {
    fn is_closed(&self) -> bool {
        self.sub_ep_hnd.is_closed()
    }

    fn close(&self, code: u32, reason: &str) {
        self.sub_ep_hnd.close(code, reason)
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        let local_addr = self.sub_ep_hnd.local_addr()?;
        let proxy_addr: TxUrl = ProxyUrl::new(local_addr.as_str(), self.cert_digest.clone())
            .map_err(KitsuneError::other)?
            .as_str()
            .into();
        Ok(proxy_addr)
    }

    fn connect(
        &self,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<ConHnd>> {
        let cert_digest = self.cert_digest.clone();
        let sub_ep_hnd = self.sub_ep_hnd.clone();
        let inner = self.inner.clone();
        async move {
            let url = ProxyUrl::from(remote.as_str());
            let digest = url.digest();
            if digest == cert_digest {
                return Err("refusing to connect to self".into());
            }

            let inner2 = inner.clone();
            let digest2 = digest.clone();
            let base: TxUrl = url.as_base().as_str().into();
            let connect = move || {
                ConRef::Pending(
                    async move {
                        match sub_ep_hnd.connect(base, timeout).await {
                            Ok(con) => {
                                ingest_outgoing_con(inner2, cert_digest, digest2, con, timeout)
                                    .await
                            }
                            Err(e) => Err(clear_outgoing_con(inner2, digest2, e).await),
                        }
                    }
                    .boxed()
                    .shared(),
                )
            };

            let r = inner
                .share_mut(|i, _| Ok(i.all_cons.entry(digest).or_insert_with(connect).clone()))?;

            match r {
                ConRef::Ready(c) => Ok(c),
                ConRef::Pending(p) => p.await,
            }
        }
        .boxed()
    }
}

async fn incoming_evt_logic(
    mut sub_ep: Ep,
    _hnd: Arc<ProxyEpHnd>,
    logic_hnd: LogicChanHandle<EpEvent>,
) {
    while let Some(evt) = sub_ep.next().await {
        println!("RECEIVE EVENT: {:?}", evt);
        use EpEvent::*;
        if match evt {
            IncomingConnection(_) => Ok(()),
            IncomingData(_con, _msg_id, data) => {
                if data.is_empty() {
                    // TODO - FIXME - kill connection
                    panic!("corrupt incoming data");
                }
                match data[0] {
                    PROXY_HELLO_DIGEST => {}
                    // TODO - FIXME - kill connection
                    _ => panic!("corrupt incoming data"),
                }
                Ok(())
            }
            ConnectionClosed(_url, _code, _reason) => Ok(()),
            Error(e) => logic_hnd.emit(Error(e)),
            EndpointClosed => logic_hnd.emit(EndpointClosed),
        }
        .is_err()
        {
            break;
        }
    }
}

struct ProxyEp {
    logic_chan: LogicChan<EpEvent>,
    hnd: EpHnd,
}

impl ProxyEp {
    pub async fn new(sub_ep: Ep, cert_digest: CertDigest) -> KitsuneResult<Ep> {
        let logic_chan = LogicChan::new(32);
        let logic_hnd = logic_chan.handle().clone();

        let hnd = ProxyEpHnd::new(sub_ep.handle().clone(), cert_digest);

        let logic = incoming_evt_logic(sub_ep, hnd.clone(), logic_hnd);

        logic_chan.handle().capture_logic(logic).await?;

        Ok(Ep(Box::new(ProxyEp {
            logic_chan,
            hnd: EpHnd(hnd),
        })))
    }
}

impl Stream for ProxyEp {
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

impl AsEp for ProxyEp {
    fn handle(&self) -> &EpHnd {
        &self.hnd
    }
}

struct ProxyEpFactory {
    tls_config: TlsConfig,
    sub_fact: EpFactory,
}

impl ProxyEpFactory {
    pub fn new(sub_fact: BackendFactory, tls_config: TlsConfig, max_cons: usize) -> EpFactory {
        let sub_fact = tx2_promote(sub_fact, max_cons);
        EpFactory(Arc::new(ProxyEpFactory {
            tls_config,
            sub_fact,
        }))
    }
}

impl AsEpFactory for ProxyEpFactory {
    fn bind(
        &self,
        bind_spec: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<Ep>> {
        let digest = self.tls_config.cert_digest.clone();
        let fut = self.sub_fact.bind(bind_spec, timeout);
        async move {
            let sub_ep = fut.await?;
            ProxyEp::new(sub_ep, digest).await
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_tx2_proxy() {
        let t = KitsuneTimeout::from_millis(5000);

        let f = tx2_proxy(
            MemBackendAdapt::new(),
            TlsConfig::new_ephemeral().await.unwrap(),
            32,
        );
        let mut ep1 = f.bind("none:", t).await.unwrap();
        let ep1hnd = ep1.handle().clone();
        let addr1 = ep1hnd.local_addr().unwrap();
        println!("got addr1: {}", addr1);

        let rt1 = tokio::task::spawn(async move {
            while let Some(evt) = ep1.next().await {
                println!("RT1: {:?}", evt);
            }
        });

        let f = tx2_proxy(
            MemBackendAdapt::new(),
            TlsConfig::new_ephemeral().await.unwrap(),
            32,
        );
        let mut ep2 = f.bind("none:", t).await.unwrap();
        let ep2hnd = ep2.handle().clone();
        let addr2 = ep2hnd.local_addr().unwrap();
        println!("got addr2: {}", addr2);

        let rt2 = tokio::task::spawn(async move {
            while let Some(evt) = ep2.next().await {
                println!("RT2: {:?}", evt);
            }
        });

        let _con = ep1hnd.connect(addr2, t).await.unwrap();

        tokio::time::delay_for(std::time::Duration::from_millis(500)).await;

        ep1hnd.close(0, "");
        ep2hnd.close(0, "");

        rt1.await.unwrap();
        rt2.await.unwrap();
    }
}

/*
use crate::{ProxyUrl, TlsConfig};
use futures::future::{BoxFuture, FutureExt};
use futures::stream::StreamExt;
use kitsune_p2p_types::tx2::tx_backend::*;
use kitsune_p2p_types::tx2::util::*;
use kitsune_p2p_types::*;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

struct ProxyConRecvAdapt {
    actor: Actor<ConFut>,
    pending: Vec<ConFut>,
}

impl ProxyConRecvAdapt {
    pub fn new(actor: Actor<ConFut>) -> Box<dyn ConRecvAdapt> {
        Box::new(Self {
            actor,
            pending: Vec::new(),
        })
    }
}

impl ConRecvAdapt for ProxyConRecvAdapt {
    fn next(&mut self) -> ConFutFut {
        async move {
            if !self.pending.is_empty() {
                return Ok(self.pending.remove(0));
            }
            let mut items = match self.actor.next().await {
                None => return Err(KitsuneError::Closed),
                Some(items) => items,
            };
            self.pending.append(&mut items);
            Ok(self.pending.remove(0))
        }
        .boxed()
    }
}

struct ProxyEndpointInner {
    actor: ActorHandle<ConFut>,
    sub_ep: Arc<dyn EndpointAdapt>,
    digest: crate::CertDigest,
    max_connections: Arc<Semaphore>,
}

struct ProxyEndpointAdapt(Mutex<Option<ProxyEndpointInner>>);

impl ProxyEndpointAdapt {
    pub fn new(
        actor: ActorHandle<ConFut>,
        sub_ep: Arc<dyn EndpointAdapt>,
        digest: crate::CertDigest,
        max_connections: Arc<Semaphore>,
    ) -> Arc<Self> {
        Arc::new(Self(Mutex::new(Some(ProxyEndpointInner {
            actor,
            sub_ep,
            digest,
            max_connections,
        }))))
    }
}

impl EndpointAdapt for ProxyEndpointAdapt {
    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        let (ep, digest) = {
            let mut lock = self.0.lock();
            if lock.is_none() {
                return Err(KitsuneError::Closed);
            }
            if lock.as_ref().unwrap().actor.is_closed() {
                *lock = None;
                return Err(KitsuneError::Closed);
            }
            let inner = lock.as_ref().unwrap();
            (inner.sub_ep.clone(), inner.digest.clone())
        };

        let local_addr = ep.local_addr()?;
        let proxy_addr: TxUrl = ProxyUrl::new(local_addr.as_str(), digest)
            .map_err(KitsuneError::other)?
            .as_str()
            .into();
        Ok(proxy_addr)
    }

    fn connect(&self, url: TxUrl, timeout: KitsuneTimeout) -> ConFut {
        let (ep, max_connections) = {
            let mut lock = self.0.lock();
            if lock.is_none() {
                return async move { Err(KitsuneError::Closed) }.boxed();
            }
            if lock.as_ref().unwrap().actor.is_closed() {
                *lock = None;
                return async move { Err(KitsuneError::Closed) }.boxed();
            }
            let inner = lock.as_ref().unwrap();
            (inner.sub_ep.clone(), inner.max_connections.clone())
        };
        async move {
            let _permit = max_connections.acquire_owned().await;
            let base_url = ProxyUrl::from(url.as_str()).into_base().as_str().into();
            let (con, _in_chan_recv) = ep.connect(base_url, timeout).await?;
            println!("got remote con addr: {}", con.remote_addr().unwrap());
            unimplemented!()
        }
        .boxed()
    }

    fn close(&self) -> BoxFuture<'static, ()> {
        let mut lock = self.0.lock();
        if lock.is_none() {
            return async move {}.boxed();
        }
        let fut = lock.as_ref().unwrap().sub_ep.close();
        *lock = None;
        fut
    }
}

/// Proxy protocol overlay for given sub-transport.
pub struct ProxyBackendAdapt {
    tls_config: TlsConfig,
    sub_transport_factory: BackendFactory,
    max_connections: Arc<Semaphore>,
}

impl ProxyBackendAdapt {
    /// Construct a new proxy protocol overlay
    pub fn new(
        tls_config: TlsConfig,
        sub_transport_factory: BackendFactory,
        max_connections: usize,
    ) -> BackendFactory {
        let out: BackendFactory = Arc::new(Self {
            tls_config,
            sub_transport_factory,
            max_connections: Arc::new(Semaphore::new(max_connections)),
        });
        out
    }
}

async fn in_con_logic(
    _ep: Arc<ProxyEndpointAdapt>,
    sub_con_recv: Box<dyn ConRecvAdapt>,
    max_connections: Arc<Semaphore>,
) {
    type P = (
        OwnedSemaphorePermit,
        Arc<dyn ConAdapt>,
        Box<dyn InChanRecvAdapt>,
    );
    type RP = BoxFuture<'static, KitsuneResult<P>>;
    type SP = futures::stream::BoxStream<'static, RP>;
    let sub_con_recv: SP = futures::stream::unfold(sub_con_recv, move |mut sub_con_recv| {
        let max_connections = max_connections.clone();
        async move {
            let permit = max_connections.acquire_owned().await;
            match sub_con_recv.next().await {
                Err(_) => None,
                Ok(fut) => Some((
                    async move {
                        let (con, chan_recv) = fut.await?;
                        Ok((permit, con, chan_recv))
                    }
                    .boxed(),
                    sub_con_recv,
                )),
            }
        }
    })
    .boxed();
    sub_con_recv
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

impl BackendAdapt for ProxyBackendAdapt {
    fn bind(&self, url: TxUrl, timeout: KitsuneTimeout) -> EndpointFut {
        let digest = self.tls_config.cert_digest.clone();
        let fut = self.sub_transport_factory.bind(url, timeout);
        let max_connections = self.max_connections.clone();
        async move {
            let (ep, sub_con_recv) = fut.await?;

            let actor_recv = Actor::new(32);

            let ep = ProxyEndpointAdapt::new(
                actor_recv.handle().clone(),
                ep,
                digest,
                max_connections.clone(),
            );

            let dyn_ep: Arc<dyn EndpointAdapt> = ep.clone();

            actor_recv
                .handle()
                .capture_logic(in_con_logic(ep, sub_con_recv, max_connections))
                .await;

            let con_recv = ProxyConRecvAdapt::new(actor_recv);

            Ok((dyn_ep, con_recv))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kitsune_p2p_types::tx2::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_proxy_backend() {
        let t = KitsuneTimeout::from_millis(5000);

        let back = ProxyBackendAdapt::new(
            TlsConfig::new_ephemeral().await.unwrap(),
            MemBackendAdapt::new(),
            32,
        );
        let (ep1, _con_recv1) = back.bind("none:".into(), t).await.unwrap();

        let back = ProxyBackendAdapt::new(
            TlsConfig::new_ephemeral().await.unwrap(),
            MemBackendAdapt::new(),
            32,
        );
        let (ep2, mut con_recv2) = back.bind("none:".into(), t).await.unwrap();

        let rt = tokio::task::spawn(async move {
            let mut all = Vec::new();
            while let Ok(fut) = con_recv2.next().await {
                if let Ok((con2, mut chan_recv2)) = fut.await {
                    let mut out_chan = con2.out_chan(t).await.unwrap();
                    all.push(tokio::task::spawn(async move {
                        while let Ok(fut) = chan_recv2.next().await {
                            if let Ok(mut in_chan) = fut.await {
                                let (_, mut buf) = in_chan.read(t).await.unwrap().remove(0);
                                println!("GOT IN CHAN!: {}", String::from_utf8_lossy(&buf[..]));
                                assert_eq!(b"hello", &buf[..]);
                                buf.clear();
                                buf.extend_from_slice(b"world");
                                out_chan.write(0.into(), buf, t).await.unwrap();
                            }
                        }
                    }));
                }
            }
            futures::future::try_join_all(all).await.unwrap();
        });

        let addr2 = ep2.local_addr().unwrap();
        println!("binding2: {}", addr2);

        let (_con1, _in_chan_recv1) = ep1.connect(addr2, t).await.unwrap();

        ep1.close().await;
        ep2.close().await;

        rt.await.unwrap();
    }
}
*/
