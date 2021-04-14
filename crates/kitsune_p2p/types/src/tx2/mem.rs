#![allow(clippy::new_ret_no_self)]
#![allow(clippy::never_loop)]

use crate::config::*;
use crate::tls::*;
use crate::tx2::tx2_adapter::*;
use crate::tx2::tx2_utils::*;
use crate::tx2::*;
use crate::*;
use futures::{
    future::{BoxFuture, FutureExt},
    stream::{BoxStream, StreamExt},
};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic;

/// Configuration for MemBackendAdapt
#[non_exhaustive]
pub struct MemConfig {
    /// Tls config
    /// Default: None = ephemeral.
    pub tls: Option<TlsConfig>,

    /// Tuning Params
    /// Default: None = default.
    pub tuning_params: Option<KitsuneP2pTuningParams>,
}

impl Default for MemConfig {
    fn default() -> Self {
        Self {
            tls: None,
            tuning_params: None,
        }
    }
}

impl MemConfig {
    /// into inner contents with default application
    pub async fn split(self) -> KitsuneResult<(TlsConfig, KitsuneP2pTuningParams)> {
        let MemConfig { tls, tuning_params } = self;

        let tls = match tls {
            None => TlsConfig::new_ephemeral().await?,
            Some(tls) => tls,
        };

        let tuning_params = tuning_params.unwrap_or_else(KitsuneP2pTuningParams::default);

        Ok((tls, tuning_params))
    }
}

/// Construct a new memory-based test endpoint adapter for kitsune tx2.
pub async fn tx2_mem_adapter(config: MemConfig) -> KitsuneResult<AdapterFactory> {
    MemBackendAdapt::new(config).await
}

// -- private -- //

static NEXT_MEM_ID: atomic::AtomicU64 = atomic::AtomicU64::new(1);

type ChanSend = TSender<InChan>;
type ChanRecv = TReceiver<InChan>;
type ConSend = TSender<Con>;
type ConRecv = TReceiver<Con>;

type EndpointItem = (ConSend, Active, Tx2Cert);
static MEM_ENDPOINTS: Lazy<Mutex<HashMap<u64, EndpointItem>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

struct MemInChanRecvAdapt(BoxStream<'static, InChanFut>);

impl MemInChanRecvAdapt {
    pub fn new(recv: ChanRecv, active: Active) -> Self {
        Self(
            futures::stream::unfold((recv, active), move |(mut recv, active)| async move {
                let fut = active.fut(async move {
                    let item = recv
                        .next()
                        .await
                        .ok_or_else(|| KitsuneError::from(KitsuneErrorKind::Closed))?;
                    Ok((item, recv))
                });
                match fut.await {
                    Err(_) => None,
                    Ok((item, recv)) => Some((async move { Ok(item) }.boxed(), (recv, active))),
                }
            })
            .boxed(),
        )
    }
}

impl futures::stream::Stream for MemInChanRecvAdapt {
    type Item = InChanFut;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let inner = &mut self.0;
        tokio::pin!(inner);
        futures::stream::Stream::poll_next(inner, cx)
    }
}

impl InChanRecvAdapt for MemInChanRecvAdapt {}

struct MemConAdaptInner {
    uniq: Uniq,
    peer_addr: TxUrl,
    peer_cert: Tx2Cert,
    chan_send: ChanSend,
    con_active: Active,
    mix_active: Active,
}

struct MemConAdapt(MemConAdaptInner);

impl MemConAdapt {
    fn new(
        peer_addr: TxUrl,
        peer_cert: Tx2Cert,
        chan_send: ChanSend,
        con_active: Active,
        mix_active: Active,
    ) -> Self {
        Self(MemConAdaptInner {
            uniq: Uniq::default(),
            peer_addr,
            peer_cert,
            chan_send,
            con_active,
            mix_active,
        })
    }
}

impl ConAdapt for MemConAdapt {
    fn uniq(&self) -> Uniq {
        self.0.uniq
    }

    fn peer_addr(&self) -> KitsuneResult<TxUrl> {
        Ok(self.0.peer_addr.clone())
    }

    fn peer_cert(&self) -> KitsuneResult<Tx2Cert> {
        Ok(self.0.peer_cert.clone())
    }

    fn out_chan(&self, _timeout: KitsuneTimeout) -> OutChanFut {
        let sender = self.0.chan_send.clone();
        let (send, recv) = bound_async_mem_channel(4096, Some(&self.0.mix_active));
        async move {
            let send: OutChan = Box::new(FramedWriter::new(send));
            let recv: InChan = Box::new(FramedReader::new(recv));
            if sender.send(recv).await.is_err() {
                return Err("failed to create out channel".into());
            }
            Ok(send)
        }
        .boxed()
    }

    fn is_closed(&self) -> bool {
        !self.0.mix_active.is_active()
    }

    fn close(&self, _code: u32, _reason: &str) -> BoxFuture<'static, ()> {
        self.0.con_active.kill();
        self.0.chan_send.clone().close_channel();
        async move {}.boxed()
    }
}

struct MemConRecvAdapt(BoxStream<'static, ConFut>);

impl MemConRecvAdapt {
    pub fn new(recv: ConRecv, active: Active) -> Self {
        Self(
            futures::stream::unfold((recv, active), move |(mut recv, active)| async move {
                let fut = active.fut(async move {
                    let item = recv
                        .next()
                        .await
                        .ok_or_else(|| KitsuneError::from(KitsuneErrorKind::Closed))?;
                    Ok((item, recv))
                });
                match fut.await {
                    Err(_) => None,
                    Ok((item, recv)) => Some((async move { Ok(item) }.boxed(), (recv, active))),
                }
            })
            .boxed(),
        )
    }
}

impl futures::stream::Stream for MemConRecvAdapt {
    type Item = ConFut;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let inner = &mut self.0;
        tokio::pin!(inner);
        futures::stream::Stream::poll_next(inner, cx)
    }
}

impl ConRecvAdapt for MemConRecvAdapt {}

struct MemEndpointAdaptInner {
    id: u64,
    local_cert: Tx2Cert,
    url: TxUrl,
    ep_active: Active,
    c_send: ConSend,
}

impl Drop for MemEndpointAdaptInner {
    fn drop(&mut self) {
        MEM_ENDPOINTS.lock().remove(&self.id);
    }
}

struct MemEndpointAdapt(Mutex<MemEndpointAdaptInner>, Uniq);

impl MemEndpointAdapt {
    pub fn new(c_send: ConSend, id: u64, local_cert: Tx2Cert) -> (Self, Active) {
        let url = format!("kitsune-mem://{}", id);
        let ep_active = Active::new();
        (
            Self(
                Mutex::new(MemEndpointAdaptInner {
                    id,
                    local_cert,
                    url: url.into(),
                    ep_active: ep_active.clone(),
                    c_send,
                }),
                Uniq::default(),
            ),
            ep_active,
        )
    }
}

impl EndpointAdapt for MemEndpointAdapt {
    fn debug(&self) -> serde_json::Value {
        let inner = self.0.lock();
        if inner.ep_active.is_active() {
            serde_json::json!({
                "type": "tx2_mem",
                "state": "open",
                "addr": &inner.url,
            })
        } else {
            serde_json::json!({
                "type": "tx2_mem",
                "state": "closed",
            })
        }
    }

    fn uniq(&self) -> Uniq {
        self.1
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        let inner = self.0.lock();
        if !inner.ep_active.is_active() {
            return Err(KitsuneErrorKind::Closed.into());
        }
        Ok(inner.url.clone())
    }

    fn local_cert(&self) -> KitsuneResult<Tx2Cert> {
        let inner = self.0.lock();
        if !inner.ep_active.is_active() {
            return Err(KitsuneErrorKind::Closed.into());
        }
        Ok(inner.local_cert.clone())
    }

    fn connect(&self, url: TxUrl, timeout: KitsuneTimeout) -> ConFut {
        let (this_url, local_cert, this_ep_active) = {
            let inner = self.0.lock();
            if !inner.ep_active.is_active() {
                return async move { Err(KitsuneErrorKind::Closed.into()) }.boxed();
            }
            (
                inner.url.clone(),
                inner.local_cert.clone(),
                inner.ep_active.clone(),
            )
        };
        async move {
            let con_id = NEXT_MEM_ID.fetch_add(1, atomic::Ordering::Relaxed);

            let bad_url = || Err(format!("invalid url: {}", url).into());

            if url.scheme() != "kitsune-mem" {
                return bad_url();
            }

            let id = match url.host_str() {
                None => return bad_url(),
                Some(id) => id,
            };

            let id = match id.parse::<u64>() {
                Err(_) => return bad_url(),
                Ok(id) => id,
            };

            let (c_send, oth_ep_active, remote_cert) = match MEM_ENDPOINTS.lock().get(&id) {
                None => return Err(format!("remote not found: {}", url).into()),
                Some((s, a, d)) => (s.clone(), a.clone(), d.clone()),
            };

            let con_active = Active::new();
            let mix_ep_active = this_ep_active.mix(&oth_ep_active);
            let mix_active = con_active.mix(&mix_ep_active);

            let (send, oth_recv) = t_chan(1);
            let (oth_send, recv) = t_chan(1);

            let oth_con = MemConAdapt::new(
                format!("{}/{}", this_url, con_id).into(),
                local_cert,
                oth_send,
                con_active.clone(),
                mix_active.clone(),
            );
            let oth_con: Arc<dyn ConAdapt> = Arc::new(oth_con);

            let con = MemConAdapt::new(
                format!("{}/{}", url, con_id).into(),
                remote_cert,
                send,
                con_active,
                mix_active.clone(),
            );
            let con: Arc<dyn ConAdapt> = Arc::new(con);

            let oth_chan_recv: Box<dyn InChanRecvAdapt> =
                Box::new(MemInChanRecvAdapt::new(oth_recv, mix_active.clone()));
            let chan_recv: Box<dyn InChanRecvAdapt> =
                Box::new(MemInChanRecvAdapt::new(recv, mix_active));

            use futures::future::TryFutureExt;
            if timeout
                .mix(
                    c_send
                        .send((oth_con, oth_chan_recv))
                        .map_err(|_| KitsuneError::from(KitsuneErrorKind::Closed)),
                )
                .await
                .is_err()
            {
                MEM_ENDPOINTS.lock().remove(&id);
                return Err(format!("failed to establish connection: {}", url).into());
            }

            Ok((con, chan_recv))
        }
        .boxed()
    }

    fn is_closed(&self) -> bool {
        !self.0.lock().ep_active.is_active()
    }

    fn close(&self, _code: u32, _reason: &str) -> BoxFuture<'static, ()> {
        let lock = self.0.lock();
        lock.ep_active.kill();
        lock.c_send.close_channel();
        async move {}.boxed()
    }
}

/// Memory-based test endpoint adapter for kitsune tx2.
struct MemBackendAdapt(Tx2Cert);

impl MemBackendAdapt {
    /// Construct a new memory-based test endpoint adapter for kitsune tx2.
    pub async fn new(config: MemConfig) -> KitsuneResult<AdapterFactory> {
        let (tls, _tuning_params) = config.split().await?;
        let out: AdapterFactory = Arc::new(Self(tls.cert_digest.into()));
        Ok(out)
    }
}

impl BindAdapt for MemBackendAdapt {
    fn bind(&self, _url: TxUrl, timeout: KitsuneTimeout) -> EndpointFut {
        let local_cert = self.0.clone();
        timeout
            .mix(async move {
                let id = NEXT_MEM_ID.fetch_add(1, atomic::Ordering::Relaxed);
                let (c_send, c_recv) = t_chan(32);
                let (ep, ep_active) = MemEndpointAdapt::new(c_send.clone(), id, local_cert.clone());
                MEM_ENDPOINTS
                    .lock()
                    .insert(id, (c_send, ep_active.clone(), local_cert));
                let ep: Arc<dyn EndpointAdapt> = Arc::new(ep);
                let rc: Box<dyn ConRecvAdapt> = Box::new(MemConRecvAdapt::new(c_recv, ep_active));
                Ok((ep, rc))
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn hnd_con(
        c: Con,
        r_send: TSender<()>,
        w_send: TSender<tokio::task::JoinHandle<()>>,
    ) -> Arc<dyn ConAdapt> {
        let t = KitsuneTimeout::from_millis(5000);

        let (con, chan_recv) = c;
        let con2 = con.clone();
        w_send
            .send(tokio::task::spawn(async move {
                let con2 = &con2;
                let r_send = &r_send;
                chan_recv
                    .for_each_concurrent(8, |recv| async move {
                        let mut recv = recv.await.unwrap();
                        while let Ok((_, mut buf)) = recv.read(t).await {
                            if &*buf == b"hello" {
                                let mut out_chan = con2.out_chan(t).await.unwrap();
                                buf.clear();
                                buf.extend_from_slice(b"world");
                                out_chan.write(0.into(), buf, t).await.unwrap();
                            } else if &*buf == b"world" {
                                if r_send.clone().send(()).await.is_err() {
                                    return;
                                }
                            } else {
                                panic!("unexpected {}", String::from_utf8_lossy(&*buf));
                            }
                        }
                    })
                    .await;
            }))
            .await
            .unwrap();
        con
    }

    async fn mk_node(
        f: &AdapterFactory,
        r_send: TSender<()>,
        w_send: TSender<tokio::task::JoinHandle<()>>,
    ) -> (TxUrl, Arc<dyn EndpointAdapt>) {
        let t = KitsuneTimeout::from_millis(5000);

        let (ep, con_recv) = f.bind("none:".into(), t).await.unwrap();
        let w_send2 = w_send.clone();
        w_send
            .send(tokio::task::spawn(async move {
                let r_send = &r_send;
                let w_send2 = &w_send2;
                con_recv
                    .for_each_concurrent(8, |c| async move {
                        hnd_con(c.await.unwrap(), r_send.clone(), w_send2.clone()).await;
                    })
                    .await;
            }))
            .await
            .unwrap();

        let addr = ep.local_addr().unwrap();

        (addr, ep)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx2_mem_stress() {
        let t = KitsuneTimeout::from_millis(5000);

        const COUNT: usize = 100;

        let f = MemBackendAdapt::new(MemConfig::default()).await.unwrap();
        let (r_send, mut r_recv) = t_chan(COUNT * 3);
        let (w_send, w_recv) = t_chan(COUNT * 3);

        let (addr, ep) = mk_node(&f, r_send.clone(), w_send.clone()).await;

        let mut nodes = Vec::new();

        for _ in 0..COUNT {
            let (_, ep) = mk_node(&f, r_send.clone(), w_send.clone()).await;

            let con = hnd_con(
                ep.connect(addr.clone(), t).await.unwrap(),
                r_send.clone(),
                w_send.clone(),
            )
            .await;

            nodes.push((ep, con));
        }

        let mut reqs = Vec::new();

        for (_, con) in nodes.iter() {
            let out_chan_fut = con.out_chan(t);
            reqs.push(async move {
                let mut out_chan = out_chan_fut.await.unwrap();
                let mut buf = PoolBuf::new();
                buf.extend_from_slice(b"hello");
                out_chan.write(0.into(), buf, t).await.unwrap();
            });
        }

        futures::future::join_all(reqs).await;

        for _ in 0..COUNT {
            let _ = r_recv.next().await;
        }

        ep.close(0, "").await;
        for (ep, _) in nodes.iter() {
            ep.close(0, "").await;
        }

        w_send.close_channel();

        futures::future::try_join_all(w_recv.collect::<Vec<_>>().await)
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx2_mem() {
        let t = KitsuneTimeout::from_millis(5000);

        let back = MemBackendAdapt::new(MemConfig::default()).await.unwrap();
        let (ep1, _con_recv1) = back.bind("none:".into(), t).await.unwrap();
        let (ep2, mut con_recv2) = back.bind("none:".into(), t).await.unwrap();

        let rt = tokio::task::spawn(async move {
            let mut all = Vec::new();
            while let Some(fut) = con_recv2.next().await {
                println!("in-con-1");
                if let Ok((con2, mut chan_recv2)) = fut.await {
                    println!("in-con-2");

                    let mut out_chan = con2.out_chan(t).await.unwrap();
                    all.push(tokio::task::spawn(async move {
                        println!("in-chan-1");
                        while let Some(fut) = chan_recv2.next().await {
                            println!("in-chan-2");
                            if let Ok(mut in_chan) = fut.await {
                                let (_, mut buf) = in_chan.read(t).await.unwrap();
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
            println!("RECV LOOP ABORT");
        });

        let addr2 = ep2.local_addr().unwrap();
        println!("addr2 = {}", addr2);

        let (con1, mut chan_recv1) = ep1.connect(addr2, t).await.unwrap();
        println!("con to = {}", con1.peer_addr().unwrap());

        let mut out_chan = con1.out_chan(t).await.unwrap();
        let mut buf = PoolBuf::new();
        buf.extend_from_slice(b"hello");
        out_chan.write(0.into(), buf, t).await.unwrap();

        let mut in_chan = chan_recv1.next().await.unwrap().await.unwrap();
        let (_, buf) = in_chan.read(t).await.unwrap();
        println!("GOT RESPONSE: {}", String::from_utf8_lossy(&buf[..]));
        assert_eq!(b"world", &buf[..]);

        ep1.close(0, "");
        ep2.close(0, "");

        rt.await.unwrap();
    }
}
