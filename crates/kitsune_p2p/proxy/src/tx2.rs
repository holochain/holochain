//! Next-gen performance kitsune transport proxy

use crate::{TlsConfig, ProxyUrl};
use kitsune_p2p_types::*;
use kitsune_p2p_types::tx2::tx_backend::*;
use kitsune_p2p_types::tx2::util::TxUrl;
use std::sync::Arc;
use futures::future::{BoxFuture, FutureExt};
use parking_lot::Mutex;

struct ProxyConRecvAdapt;

impl ProxyConRecvAdapt {
    pub fn new() -> Box<dyn ConRecvAdapt> {
        Box::new(Self)
    }
}

impl ConRecvAdapt for ProxyConRecvAdapt {
    fn next(&mut self) -> ConFutFut {
        unimplemented!()
    }
}

struct ProxyEndpointInner {
    this_url: TxUrl,
    sub_ep: Arc<dyn EndpointAdapt>,
}

struct ProxyEndpointAdapt(Arc<Mutex<Option<ProxyEndpointInner>>>);

impl ProxyEndpointAdapt {
    pub fn new(
        this_url: TxUrl,
        sub_ep: Arc<dyn EndpointAdapt>,
    ) -> Arc<dyn EndpointAdapt> {
        Arc::new(Self(Arc::new(Mutex::new(Some(ProxyEndpointInner {
            this_url,
            sub_ep,
        })))))
    }
}

impl EndpointAdapt for ProxyEndpointAdapt {
    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        let lock = self.0.lock();
        if lock.is_none() {
            return Err(KitsuneError::Closed);
        }
        Ok(lock.as_ref().unwrap().this_url.clone())
    }

    fn connect(&self, url: TxUrl, timeout: KitsuneTimeout) -> ConFut {
        let base_url = ProxyUrl::from(url.as_str()).into_base().as_str().into();
        let fut = {
            let lock = self.0.lock();
            if lock.is_none() {
                return async move { Err(KitsuneError::Closed) }.boxed();
            }
            lock.as_ref().unwrap().sub_ep.connect(base_url, timeout)
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
        if let Some(inner) = lock.take() {
            inner.sub_ep.close()
        } else {
            async move { }.boxed()
        }
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
            let (ep, _con_recv) = fut.await?;
            let local_addr = ep.local_addr()?;
            let proxy_addr: TxUrl = ProxyUrl::new(local_addr.as_str(), digest).map_err(KitsuneError::other)?.as_str().into();

            let ep = ProxyEndpointAdapt::new(proxy_addr, ep);
            let con_recv = ProxyConRecvAdapt::new();

            Ok((ep, con_recv))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kitsune_p2p_types::tx2::*;
    use futures::io::{AsyncReadExt, AsyncWriteExt};

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
                                let mut bob = [0_u8; 5];
                                in_chan.read_exact(&mut bob).await.unwrap();
                                println!("GOT IN CHAN!: {}", String::from_utf8_lossy(&bob[..]));
                                assert_eq!(b"hello", &bob[..]);
                                out_chan.write_all(b"world").await.unwrap();
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
