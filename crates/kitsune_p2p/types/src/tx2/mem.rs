#![allow(clippy::new_ret_no_self)]

use crate::tx2::*;
use crate::tx2::tx_backend::*;
use crate::*;
use futures::{future::{BoxFuture, FutureExt}, stream::StreamExt};
use parking_lot::Mutex;
use std::sync::atomic;
use once_cell::sync::Lazy;
use std::collections::HashMap;

static NEXT_MEM_ID: atomic::AtomicU64 = atomic::AtomicU64::new(1);

type ChanSend = tokio::sync::mpsc::Sender<InChan>;
type ChanRecv = tokio::sync::mpsc::Receiver<InChan>;
type ConSend = tokio::sync::mpsc::Sender<Con>;
type ConRecv = tokio::sync::mpsc::Receiver<Con>;

#[derive(Debug, Clone)]
struct Active([Option<Arc<atomic::AtomicBool>>; 4]);

impl Active {
    pub fn new() -> Self {
        Self([
            Some(Arc::new(atomic::AtomicBool::new(true))),
            None,
            None,
            None,
        ])
    }

    pub fn mix(&self, oth: &Self) -> Self {
        let mut inner = self.0.clone();
        'top: for o in oth.0.iter() {
            if let Some(o) = o {
                for i in 0..4 {
                    if inner[i].is_none() {
                        inner[i] = Some(o.clone());
                        continue 'top;
                    }
                }
                panic!("No remaining Active slots");
            }
        }
        Self(inner)
    }

    pub fn kill(&self) {
        for a in self.0.iter() {
            if let Some(a) = a {
                a.store(false, atomic::Ordering::SeqCst);
            }
        }
    }

    pub fn is_active(&self) -> bool {
        for a in self.0.iter() {
            if let Some(a) = a {
                if !a.load(atomic::Ordering::SeqCst) {
                    return false;
                }
            }
        }
        return true;
    }
}

static MEM_ENDPOINTS: Lazy<Mutex<HashMap<u64, (ConSend, Active)>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

struct MemInChanRecvAdapt(ChanRecv);

impl InChanRecvAdapt for MemInChanRecvAdapt {
    fn next(&mut self) -> InChanFutFut {
        let fut = self.0.next();
        async move {
            let chan = fut.await.ok_or_else(|| KitsuneError::Closed)?;
            Ok(async move {
                Ok(chan)
            }.boxed())
        }.boxed()
    }
}

struct MemConAdaptInner {
    remote_addr: String,
    chan_send: ChanSend,
    con_active: Active,
}

struct MemConAdapt(Arc<MemConAdaptInner>);

impl MemConAdapt {
    fn new(remote_addr: String, chan_send: ChanSend, con_active: Active) -> Self {
        Self(Arc::new(MemConAdaptInner {
            remote_addr,
            chan_send,
            con_active,
        }))
    }
}

impl ConAdapt for MemConAdapt {
    fn remote_addr(&self) -> KitsuneResult<String> {
        Ok(self.0.remote_addr.clone())
    }

    fn out_chan(&self, _timeout: KitsuneTimeout) -> OutChanFut {
        let mut sender = self.0.chan_send.clone();
        async move {
            let (send, recv) = util::bound_async_mem_channel(4096);
            if sender.send(recv).await.is_err() {
                return Err("failed to create out channel".into());
            }
            Ok(send)
        }.boxed()
    }

    fn close(&self) -> BoxFuture<'static, ()> {
        self.0.con_active.kill();
        async move {}.boxed()
    }
}

struct MemConRecvAdapt(ConRecv);

impl ConRecvAdapt for MemConRecvAdapt {
    fn next(&mut self) -> ConFutFut {
        let fut = self.0.next();
        async move {
            let con = fut.await.ok_or_else(|| KitsuneError::Closed)?;
            Ok(async move {
                Ok(con)
            }.boxed())
        }.boxed()
    }
}

struct MemEndpointAdaptInner {
    id: u64,
    url: String,
    ep_active: Active,
}

impl Drop for MemEndpointAdaptInner {
    fn drop(&mut self) {
        MEM_ENDPOINTS.lock().remove(&self.id);
    }
}

struct MemEndpointAdapt(Arc<Mutex<MemEndpointAdaptInner>>);

impl MemEndpointAdapt {
    pub fn new(id: u64) -> (Self, Active) {
        let url = format!("kitsune-mem://{}", id);
        let ep_active = Active::new();
        (
            Self(Arc::new(Mutex::new(MemEndpointAdaptInner {
                id,
                url,
                ep_active: ep_active.clone(),
            }))),
            ep_active,
        )
    }
}

impl EndpointAdapt for MemEndpointAdapt {
    fn local_addr(&self) -> KitsuneResult<String> {
        let inner = self.0.lock();
        if !inner.ep_active.is_active() {
            return Err(KitsuneError::Closed);
        }
        Ok(inner.url.clone())
    }

    fn connect(&self, url: String, _timeout: KitsuneTimeout) -> ConFut {
        let (this_url, this_ep_active) = {
            let inner = self.0.lock();
            if !inner.ep_active.is_active() {
                return async move { Err(KitsuneError::Closed) }.boxed();
            }
            (inner.url.clone(), inner.ep_active.clone())
        };
        async move {
            let con_id = NEXT_MEM_ID.fetch_add(1, atomic::Ordering::Relaxed);
            if !url.starts_with("kitsune-mem://") {
                return Err(format!("invalid url: {}", url).into());
            }
            let id: u64 = match String::from_utf8_lossy(&url.as_bytes()[14..]).parse() {
                Err(_) => return Err(format!("invalid url: {}", url).into()),
                Ok(id) => id,
            };
            let (mut sender, oth_ep_active) = match MEM_ENDPOINTS.lock().get(&id) {
                None => return Err(format!("remote not found: {}", url).into()),
                Some((s, a)) => (s.clone(), a.clone()),
            };

            let con_active = Active::new();
            let mix_ep_active = this_ep_active.mix(&oth_ep_active);
            let _mix_active = con_active.mix(&mix_ep_active);

            let (send, oth_recv) = tokio::sync::mpsc::channel(1);
            let (oth_send, recv) = tokio::sync::mpsc::channel(1);

            let oth_con = MemConAdapt::new(format!("{}/{}", this_url, con_id), oth_send, con_active.clone());
            let oth_con: Arc<dyn ConAdapt> = Arc::new(oth_con);

            let con = MemConAdapt::new(format!("{}/{}", url, con_id), send, con_active);
            let con: Arc<dyn ConAdapt> = Arc::new(con);

            let oth_chan_recv: Box<dyn InChanRecvAdapt> = Box::new(MemInChanRecvAdapt(oth_recv));
            let chan_recv: Box<dyn InChanRecvAdapt> = Box::new(MemInChanRecvAdapt(recv));

            if sender.send((oth_con, oth_chan_recv)).await.is_err() {
                MEM_ENDPOINTS.lock().remove(&id);
                return Err(format!("failed to establish connection: {}", url).into());
            }

            Ok((con, chan_recv))
        }.boxed()
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
    /// Construct a new test memory-based kitsune tx2 endpoint adapter.
    pub fn new() -> Self {
        Self
    }
}

impl BackendAdapt for MemBackendAdapt {
    fn bind(&self, _url: String, _timeout: KitsuneTimeout) -> EndpointFut {
        async move {
            let id = NEXT_MEM_ID.fetch_add(1, atomic::Ordering::Relaxed);
            let (c_send, c_recv) = tokio::sync::mpsc::channel(1);
            let (ep, ep_active) = MemEndpointAdapt::new(id);
            MEM_ENDPOINTS.lock().insert(id, (c_send, ep_active));
            let ep: Arc<dyn EndpointAdapt> = Arc::new(ep);
            let rc: Box<dyn ConRecvAdapt> = Box::new(MemConRecvAdapt(c_recv));
            Ok((ep, rc))
        }.boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_tx2_mem_backend() {
        let back = MemBackendAdapt::new();

        let (ep1, _con_recv1) = back.bind("".to_string(), KitsuneTimeout::from_millis(1000 * 30)).await.unwrap();
        let (ep2, mut con_recv2) = back.bind("".to_string(), KitsuneTimeout::from_millis(1000 * 30)).await.unwrap();

        let rt = tokio::task::spawn(async move {
            let mut all = Vec::new();
            while let Ok(fut) = con_recv2.next().await {
                if let Ok((_con2, mut chan_recv2)) = fut.await {
                    all.push(tokio::task::spawn(async move {
                        while let Ok(fut) = chan_recv2.next().await {
                            if let Ok(_in_chan) = fut.await {
                                println!("GOT IN CHAN!");
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

        let (con1, _chan_recv1) = ep1.connect(addr2, KitsuneTimeout::from_millis(1000 * 30)).await.unwrap();
        println!("con to = {}", con1.remote_addr().unwrap());

        let _out_chan = con1.out_chan(KitsuneTimeout::from_millis(1000 * 30)).await.unwrap();

        ep1.close().await;
        ep2.close().await;

        rt.await.unwrap();
    }
}
