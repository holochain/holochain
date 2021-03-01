#![allow(clippy::new_ret_no_self)]
//! Next-gen performance kitsune transport proxy

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
