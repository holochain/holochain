#![allow(clippy::new_ret_no_self)]
#![allow(clippy::never_loop)]

use crate::tx2::tx_backend::*;
use crate::tx2::util::{Active, TxUrl};
use crate::tx2::*;
use crate::*;
use futures::{
    future::{BoxFuture, FutureExt},
    stream::StreamExt,
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

struct MemInChanRecvAdapt {
    recv: ChanRecv,
    active: Active,
}

impl InChanRecvAdapt for MemInChanRecvAdapt {
    fn next(&mut self) -> InChanFutFut {
        let fut = self.recv.next();
        let fut = self
            .active
            .fut(async move { fut.await.ok_or(KitsuneError::Closed) });
        let active = &self.active;
        async move {
            let chan = fut.await?;
            Ok(active.fut(async move { Ok(chan) }).boxed())
        }
        .boxed()
    }
}

struct MemConAdaptInner {
    remote_addr: TxUrl,
    chan_send: ChanSend,
    con_active: Active,
}

struct MemConAdapt(MemConAdaptInner);

impl MemConAdapt {
    fn new(remote_addr: TxUrl, chan_send: ChanSend, con_active: Active) -> Self {
        Self(MemConAdaptInner {
            remote_addr,
            chan_send,
            con_active,
        })
    }
}

impl ConAdapt for MemConAdapt {
    fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        Ok(self.0.remote_addr.clone())
    }

    fn out_chan(&self, _timeout: KitsuneTimeout) -> OutChanFut {
        let mut sender = self.0.chan_send.clone();
        async move {
            let (send, recv) = util::bound_async_mem_channel(4096);
            let send: OutChan = Box::new(FramedWriter::new(send));
            let recv: InChan = Box::new(FramedReader::new(recv));
            if sender.send(recv).await.is_err() {
                return Err("failed to create out channel".into());
            }
            Ok(send)
        }
        .boxed()
    }

    fn close(&self) -> BoxFuture<'static, ()> {
        self.0.con_active.kill();
        async move {}.boxed()
    }
}

struct MemConRecvAdapt {
    recv: ConRecv,
    active: Active,
}

impl ConRecvAdapt for MemConRecvAdapt {
    fn next(&mut self) -> ConFutFut {
        let fut = self.recv.next();
        let fut = self
            .active
            .fut(async move { fut.await.ok_or(KitsuneError::Closed) });
        let active = &self.active;
        async move {
            let con = fut.await?;
            Ok(active.fut(async move { Ok(con) }).boxed())
        }
        .boxed()
    }
}

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

struct MemEndpointAdapt(Mutex<MemEndpointAdaptInner>);

impl MemEndpointAdapt {
    pub fn new(id: u64) -> (Self, Active) {
        let url = format!("kitsune-mem://{}", id);
        let ep_active = Active::new();
        (
            Self(Mutex::new(MemEndpointAdaptInner {
                id,
                url: url.into(),
                ep_active: ep_active.clone(),
            })),
            ep_active,
        )
    }
}

impl EndpointAdapt for MemEndpointAdapt {
    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        let inner = self.0.lock();
        if !inner.ep_active.is_active() {
            return Err(KitsuneError::Closed);
        }
        Ok(inner.url.clone())
    }

    fn connect(&self, url: TxUrl, _timeout: KitsuneTimeout) -> ConFut {
        let (this_url, this_ep_active) = {
            let inner = self.0.lock();
            if !inner.ep_active.is_active() {
                return async move { Err(KitsuneError::Closed) }.boxed();
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
            );
            let oth_con: Arc<dyn ConAdapt> = Arc::new(oth_con);

            let con = MemConAdapt::new(format!("{}/{}", url, con_id).into(), send, con_active);
            let con: Arc<dyn ConAdapt> = Arc::new(con);

            let oth_chan_recv: Box<dyn InChanRecvAdapt> = Box::new(MemInChanRecvAdapt {
                recv: oth_recv,
                active: mix_active.clone(),
            });
            let chan_recv: Box<dyn InChanRecvAdapt> = Box::new(MemInChanRecvAdapt {
                recv,
                active: mix_active,
            });

            if sender.send((oth_con, oth_chan_recv)).await.is_err() {
                MEM_ENDPOINTS.lock().remove(&id);
                return Err(format!("failed to establish connection: {}", url).into());
            }

            Ok((con, chan_recv))
        }
        .boxed()
    }

    fn close(&self) -> BoxFuture<'static, ()> {
        {
            let inner = self.0.lock();
            inner.ep_active.kill();
        }
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
            let rc: Box<dyn ConRecvAdapt> = Box::new(MemConRecvAdapt {
                recv: c_recv,
                active: ep_active,
            });
            Ok((ep, rc))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_tx2_mem_backend() {
        let t = KitsuneTimeout::from_millis(5000);

        let back = MemBackendAdapt::new();
        let (ep1, _con_recv1) = back.bind("none:".into(), t).await.unwrap();
        let (ep2, mut con_recv2) = back.bind("none:".into(), t).await.unwrap();

        let rt = tokio::task::spawn(async move {
            let mut all = Vec::new();
            while let Ok(fut) = con_recv2.next().await {
                println!("in-con-1");
                if let Ok((con2, mut chan_recv2)) = fut.await {
                    println!("in-con-2");

                    let mut out_chan = con2.out_chan(t).await.unwrap();
                    all.push(tokio::task::spawn(async move {
                        println!("in-chan-1");
                        while let Ok(fut) = chan_recv2.next().await {
                            println!("in-chan-2");
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
        let (_, buf) = in_chan.read(t).await.unwrap().remove(0);
        println!("GOT RESPONSE: {}", String::from_utf8_lossy(&buf[..]));
        assert_eq!(b"world", &buf[..]);

        ep1.close().await;
        ep2.close().await;

        rt.await.unwrap();
    }
}
