#![allow(clippy::new_ret_no_self)]
#![allow(clippy::never_loop)]

use crate::tx2::tx2_backend::*;
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

static NEXT_MEM_ID: atomic::AtomicU64 = atomic::AtomicU64::new(1);

type ChanSend = tokio::sync::mpsc::Sender<InChan>;
type ChanRecv = tokio::sync::mpsc::Receiver<InChan>;
type ConSend = tokio::sync::mpsc::Sender<Con>;
type ConRecv = tokio::sync::mpsc::Receiver<Con>;

static MEM_ENDPOINTS: Lazy<Mutex<HashMap<u64, (ConSend, Active)>>> =
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
    remote_addr: TxUrl,
    chan_send: ChanSend,
    con_active: Active,
    mix_active: Active,
}

struct MemConAdapt(MemConAdaptInner);

impl MemConAdapt {
    fn new(
        remote_addr: TxUrl,
        chan_send: ChanSend,
        con_active: Active,
        mix_active: Active,
    ) -> Self {
        Self(MemConAdaptInner {
            uniq: Uniq::default(),
            remote_addr,
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

    fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        Ok(self.0.remote_addr.clone())
    }

    fn out_chan(&self, _timeout: KitsuneTimeout) -> OutChanFut {
        let mut sender = self.0.chan_send.clone();
        async move {
            let (send, recv) = bound_async_mem_channel(4096);
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
    url: TxUrl,
    ep_active: Active,
}

impl Drop for MemEndpointAdaptInner {
    fn drop(&mut self) {
        MEM_ENDPOINTS.lock().remove(&self.id);
    }
}

struct MemEndpointAdapt(Mutex<MemEndpointAdaptInner>, Uniq);

impl MemEndpointAdapt {
    pub fn new(id: u64) -> (Self, Active) {
        let url = format!("kitsune-mem://{}", id);
        let ep_active = Active::new();
        (
            Self(
                Mutex::new(MemEndpointAdaptInner {
                    id,
                    url: url.into(),
                    ep_active: ep_active.clone(),
                }),
                Uniq::default(),
            ),
            ep_active,
        )
    }
}

impl EndpointAdapt for MemEndpointAdapt {
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

    fn connect(&self, url: TxUrl, _timeout: KitsuneTimeout) -> ConFut {
        let (this_url, this_ep_active) = {
            let inner = self.0.lock();
            if !inner.ep_active.is_active() {
                return async move { Err(KitsuneErrorKind::Closed.into()) }.boxed();
            }
            (inner.url.clone(), inner.ep_active.clone())
        };
        async move {
            let con_id = NEXT_MEM_ID.fetch_add(1, atomic::Ordering::Relaxed);
            let id: Result<u64, ()> = 'top: loop {
                if url.scheme() == "kitsune-mem" {
                    if let Some(id) = url.host_str() {
                        if let Ok(id) = id.parse::<u64>() {
                            break 'top Ok(id);
                        }
                    }
                }
                break 'top Err(());
            };
            let id = match id {
                Ok(id) => id,
                Err(_) => return Err(format!("invalid url: {}", url).into()),
            };
            let (mut sender, oth_ep_active) = match MEM_ENDPOINTS.lock().get(&id) {
                None => return Err(format!("remote not found: {}", url).into()),
                Some((s, a)) => (s.clone(), a.clone()),
            };

            let con_active = Active::new();
            let mix_ep_active = this_ep_active.mix(&oth_ep_active);
            let mix_active = con_active.mix(&mix_ep_active);

            let (send, oth_recv) = tokio::sync::mpsc::channel(1);
            let (oth_send, recv) = tokio::sync::mpsc::channel(1);

            let oth_con = MemConAdapt::new(
                format!("{}/{}", this_url, con_id).into(),
                oth_send,
                con_active.clone(),
                mix_active.clone(),
            );
            let oth_con: Arc<dyn ConAdapt> = Arc::new(oth_con);

            let con = MemConAdapt::new(
                format!("{}/{}", url, con_id).into(),
                send,
                con_active,
                mix_active.clone(),
            );
            let con: Arc<dyn ConAdapt> = Arc::new(con);

            let oth_chan_recv: Box<dyn InChanRecvAdapt> =
                Box::new(MemInChanRecvAdapt::new(oth_recv, mix_active.clone()));
            let chan_recv: Box<dyn InChanRecvAdapt> =
                Box::new(MemInChanRecvAdapt::new(recv, mix_active));

            if sender.send((oth_con, oth_chan_recv)).await.is_err() {
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
        self.0.lock().ep_active.kill();
        async move {}.boxed()
    }
}

/// Memory-based test endpoint adapter for kitsune tx2.
pub struct MemBackendAdapt;

impl MemBackendAdapt {
    /// Construct a new memory-based test endpoint adapter for kitsune tx2.
    pub fn new() -> BackendFactory {
        let out: BackendFactory = Arc::new(Self);
        out
    }
}

impl BackendAdapt for MemBackendAdapt {
    fn bind(&self, _url: TxUrl, _timeout: KitsuneTimeout) -> EndpointFut {
        async move {
            let id = NEXT_MEM_ID.fetch_add(1, atomic::Ordering::Relaxed);
            let (c_send, c_recv) = tokio::sync::mpsc::channel(1);
            let (ep, ep_active) = MemEndpointAdapt::new(id);
            MEM_ENDPOINTS.lock().insert(id, (c_send, ep_active.clone()));
            let ep: Arc<dyn EndpointAdapt> = Arc::new(ep);
            let rc: Box<dyn ConRecvAdapt> = Box::new(MemConRecvAdapt::new(c_recv, ep_active));
            Ok((ep, rc))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::sink::SinkExt;

    async fn hnd_con(
        c: Con,
        r_send: futures::channel::mpsc::Sender<()>,
        mut w_send: futures::channel::mpsc::Sender<tokio::task::JoinHandle<()>>,
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
                println!("chan recv done");
            }))
            .await
            .unwrap();
        con
    }

    async fn mk_node(
        f: &BackendFactory,
        r_send: futures::channel::mpsc::Sender<()>,
        mut w_send: futures::channel::mpsc::Sender<tokio::task::JoinHandle<()>>,
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
                println!("con recv done");
            }))
            .await
            .unwrap();

        let addr = ep.local_addr().unwrap();

        (addr, ep)
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_tx2_mem_backend_stress() {
        let t = KitsuneTimeout::from_millis(5000);

        let f = MemBackendAdapt::new();
        let (r_send, mut r_recv) = futures::channel::mpsc::channel(32);
        let (mut w_send, w_recv) = futures::channel::mpsc::channel(32);

        let (addr, ep1) = mk_node(&f, r_send.clone(), w_send.clone()).await;
        let (_, ep2) = mk_node(&f, r_send.clone(), w_send.clone()).await;

        let con = hnd_con(ep1.connect(addr, t).await.unwrap(), r_send, w_send.clone()).await;

        let mut out_chan = con.out_chan(t).await.unwrap();
        let mut buf = PoolBuf::new();
        buf.extend_from_slice(b"hello");
        out_chan.write(0.into(), buf, t).await.unwrap();

        for _ in 0..1 {
            let _ = r_recv.next().await;
        }

        ep1.close(0, "").await;
        ep2.close(0, "").await;

        println!("1");
        w_send.close().await.unwrap();
        println!("2");

        futures::future::try_join_all(w_recv.collect::<Vec<_>>().await)
            .await
            .unwrap();
        println!("3");
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_tx2_mem_backend() {
        let t = KitsuneTimeout::from_millis(5000);

        let back = MemBackendAdapt::new();
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
        println!("con to = {}", con1.remote_addr().unwrap());

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
