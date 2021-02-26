//! Next-gen performance kitsune transport proxy

use crate::{TlsConfig, ProxyUrl};
use kitsune_p2p_types::*;
use kitsune_p2p_types::tx2::tx_backend::*;
use kitsune_p2p_types::tx2::util::*;
use std::sync::Arc;
use futures::future::{BoxFuture, FutureExt};
use futures::stream::StreamExt;
use parking_lot::Mutex;

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
        }.boxed()
    }
}

struct ProxyEndpointInner {
    actor: ActorHandle<ConFut>,
    sub_ep: Arc<dyn EndpointAdapt>,
    digest: crate::CertDigest,
}

struct ProxyEndpointAdapt(Mutex<Option<ProxyEndpointInner>>);

impl ProxyEndpointAdapt {
    pub fn new(
        actor: ActorHandle<ConFut>,
        sub_ep: Arc<dyn EndpointAdapt>,
        digest: crate::CertDigest,
    ) -> Arc<dyn EndpointAdapt> {
        Arc::new(Self(Mutex::new(Some(ProxyEndpointInner {
            actor,
            sub_ep,
            digest,
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
        let proxy_addr: TxUrl = ProxyUrl::new(local_addr.as_str(), digest).map_err(KitsuneError::other)?.as_str().into();
        Ok(proxy_addr)
    }

    fn connect(&self, url: TxUrl, timeout: KitsuneTimeout) -> ConFut {
        let fut = {
            let mut lock = self.0.lock();
            if lock.is_none() {
                return async move { Err(KitsuneError::Closed) }.boxed();
            }
            if lock.as_ref().unwrap().actor.is_closed() {
                *lock = None;
                return async move { Err(KitsuneError::Closed) }.boxed();
            }
            let inner = lock.as_ref().unwrap();
            let base_url = ProxyUrl::from(url.as_str()).into_base().as_str().into();
            inner.sub_ep.connect(base_url, timeout)
        };
        async move {
            let (con, _in_chan_recv) = fut.await?;
            println!("got remote con addr: {}", con.remote_addr().unwrap());
            unimplemented!()
        }
        .boxed()
    }

    fn close(&self) -> BoxFuture<'static, ()> {
        let mut lock = self.0.lock();
        if lock.is_none() {
            return async move { }.boxed();
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
}

impl ProxyBackendAdapt {
    /// Construct a new proxy protocol overlay
    pub fn new(
        tls_config: TlsConfig,
        sub_transport_factory: BackendFactory,
    ) -> BackendFactory {
        let out: BackendFactory = Arc::new(Self {
            tls_config,
            sub_transport_factory,
        });
        out
    }
}

impl BackendAdapt for ProxyBackendAdapt {
    fn bind(&self, url: TxUrl, timeout: KitsuneTimeout) -> EndpointFut {
        let digest = self.tls_config.cert_digest.clone();
        let fut = self.sub_transport_factory.bind(url, timeout);
        async move {
            let (ep, mut con_recv) = fut.await?;

            let actor_recv = Actor::new(32);

            let ep = ProxyEndpointAdapt::new(
                actor_recv.handle().clone(),
                ep,
                digest,
            );

            let ep2 = ep.clone();
            actor_recv.handle().capture_logic(async move {
                let _ep2 = ep2;
                loop {
                    match con_recv.next().await {
                        // TODO - FIXME
                        Err(e) => panic!("{:?}", e),
                        Ok(_con_fut) => {
                            println!("GOT CON_FUT");
                            unimplemented!()
                        }
                    }
                }
            }).await;

            let con_recv = ProxyConRecvAdapt::new(actor_recv);

            Ok((ep, con_recv))
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
            MemBackendAdapt::new()
        );
        let (ep1, _con_recv1) = back.bind("none:".into(), t).await.unwrap();

        let back = ProxyBackendAdapt::new(
            TlsConfig::new_ephemeral().await.unwrap(),
            MemBackendAdapt::new()
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
