#![allow(clippy::new_ret_no_self)]
#![allow(clippy::never_loop)]
//! tx2_mem

/*
use crate::*;
use crate::tx2::*;
use crate::tx2::util::*;
use crate::tx2::tx2_backend::*;
use crate::tx2::tx2_backend::tx2_backend_traits::*;
use futures::future::{BoxFuture, FutureExt};
use futures::stream::{Stream, StreamExt};
use once_cell::sync::Lazy;
use std::sync::atomic;
use std::collections::HashMap;

static NEXT_MEM_ID: atomic::AtomicU64 = atomic::AtomicU64::new(1);
type EpRegEntry = (LogicChanHandle<EpEvent>, Active);
static EP_REG: Lazy<Share<HashMap<u64, EpRegEntry>>> = Lazy::new(|| {
    Share::new(HashMap::new())
});

struct MemConInner {
}

struct MemConHnd(Share<MemConInner>);

fn framed_chan() -> (FramedWriter, FramedReader) {
    let (send, recv) = bound_async_mem_channel(4096);
    (
        FramedWriter::new(send),
        FramedReader::new(recv),
    )
}

impl MemConHnd {
    fn new_single(
        logic_hnd: LogicChanHandle<EpEvent>,
        _con_active: Active,
        _mix_active: Active,
        _s1: FramedWriter,
        _s2: FramedWriter,
        _s3: FramedWriter,
        r1: FramedReader,
        r2: FramedReader,
        r3: FramedReader,
    ) -> ConHnd {
        // This is a calculated task spawn.
        // We could let this future be polled in the top-level endpoint
        // logic channel, but then our system would be single threaded.
        // Instead, we spawn one task per connection, gathering
        // the incoming channel data to be processed.
        tokio::task::spawn(async move {
            let mut r = futures::stream::select_all(vec![r1, r2, r3].into_iter().map(|r| {
                futures::stream::StreamExt::boxed(futures::stream::unfold(r, |mut r| async move {
                    let t = KitsuneTimeout::from_millis(1000 * 30);
                    let i = match r.read(t).await {
                        Ok(i) => i,
                        Err(_) => return None,
                    };
                    Some((i, r))
                }))
            }));

            while let Some((msg_id, data)) = r.next().await {
                if let Err(e) = logic_hnd.emit(EpEvent::IncomingData(
                    // TODO - FIXME - we need the other half here...
                    ConHnd(Arc::new(Self(Share::new(MemConInner {
                    })))),
                    msg_id,
                    data,
                )) {
                    // TODO - FIXME
                    panic!("{:?}", e);
                }
            }
        });

        ConHnd(Arc::new(Self(Share::new(MemConInner {
        }))))
    }

    pub fn new(
        logic_hnd1: LogicChanHandle<EpEvent>,
        ep_active1: Active,
        logic_hnd2: LogicChanHandle<EpEvent>,
        ep_active2: Active,
    ) -> (ConHnd, ConHnd) {
        //let sub_id = NEXT_MEM_ID.fetch_add(1, atomic::Ordering::Relaxed);

        let (s1_1, r2_1) = framed_chan();
        let (s1_2, r2_2) = framed_chan();
        let (s1_3, r2_3) = framed_chan();

        let (s2_1, r1_1) = framed_chan();
        let (s2_2, r1_2) = framed_chan();
        let (s2_3, r1_3) = framed_chan();

        let con_active = Active::new();
        let mix_active = ep_active1.mix(&ep_active2).mix(&con_active);

        let one = Self::new_single(logic_hnd1, con_active.clone(), mix_active.clone(), s1_1, s1_2, s1_3, r1_1, r1_2, r1_3);
        let two = Self::new_single(logic_hnd2, con_active, mix_active, s2_1, s2_2, s2_3, r2_1, r2_2, r2_3);

        (one, two)
    }
}

impl AsConHnd for MemConHnd {
    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, _code: u32, _reason: &str) {
        let _ = self.0.share_mut(|_, c| {
            *c = true;
            Ok(())
        });
    }

    fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        unimplemented!()
    }

    fn write(
        &self,
        _msg_id: MsgId,
        _data: PoolBuf,
        _timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        unimplemented!()
    }
}

struct MemEpInner {
    id: u64,
    url: TxUrl,
    logic_hnd: LogicChanHandle<EpEvent>,
    ep_active: Active,
}

impl Drop for MemEpInner {
    fn drop(&mut self) {
        let _ = EP_REG.share_mut(|i, _| {
            i.remove(&self.id);
            self.ep_active.kill();
            Ok(())
        });
    }
}

struct MemEpHnd(Share<MemEpInner>);

impl MemEpHnd {
    pub fn new(logic_hnd: LogicChanHandle<EpEvent>) -> EpHnd {
        let id = NEXT_MEM_ID.fetch_add(1, atomic::Ordering::Relaxed);
        let url = format!("kitsune-mem://{}", id).into();
        let ep_active = Active::new();

        let _ = EP_REG.share_mut(|i, _| {
            i.insert(id, (logic_hnd.clone(), ep_active.clone()));
            Ok(())
        });

        let inner = Share::new(MemEpInner {
            id,
            url,
            logic_hnd,
            ep_active,
        });

        let hnd: EpHnd = EpHnd(Arc::new(Self(inner)));
        hnd
    }
}

impl AsEpHnd for MemEpHnd {
    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, _code: u32, _reason: &str) {
        let _ = self.0.share_mut(|_, c| {
            *c = true;
            Ok(())
        });
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        self.0.share_mut(|i, _| Ok(i.url.clone()))
    }

    fn connect(
        &self,
        remote: TxUrl,
        _timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<ConHnd>> {
        let r = self.0.share_mut(|i, _| {
            Ok((i.logic_hnd.clone(), i.ep_active.clone()))
        });
        async move {
            let (my_logic_hnd, my_ep_active) = r?;

            let id: Result<u64, ()> = 'top: loop {
                if remote.scheme() == "kitsune-mem" {
                    if let Some(id) = remote.host_str() {
                        if let Ok(id) = id.parse::<u64>() {
                            break 'top Ok(id);
                        }
                    }
                }
                break 'top Err(());
            };

            let id = match id {
                Ok(id) => id,
                Err(_) => return Err(format!("invalid url: {}", remote).into()),
            };

            let (logic_hnd, ep_active) = match EP_REG.share_mut(|i, _| {
                i.get(&id).cloned().ok_or("".into())
            }) {
                Err(_) => return Err(format!("con refused: {}", remote).into()),
                Ok(r) => r,
            };

            let (con1, con2) = MemConHnd::new(
                my_logic_hnd,
                my_ep_active,
                logic_hnd.clone(),
                ep_active,
            );

            logic_hnd.emit(EpEvent::IncomingConnection(con2))?;

            Ok(con1)
        }.boxed()
    }
}

struct MemEp {
    hnd: EpHnd,
    logic_chan: LogicChan<EpEvent>,
}

impl MemEp {
    pub fn new() -> Ep {
        let logic_chan = LogicChan::new(32);
        let logic_hnd = logic_chan.handle().clone();
        let hnd = MemEpHnd::new(logic_hnd);
        let inner = MemEp {
            hnd,
            logic_chan,
        };
        let ep: Ep = Ep(Box::new(inner));
        ep
    }
}

impl Stream for MemEp {
    type Item = EpEvent;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>
    ) -> std::task::Poll<Option<Self::Item>> {
        let chan = &mut self.logic_chan;
        futures::pin_mut!(chan);
        Stream::poll_next(chan, cx)
    }
}

impl AsEp for MemEp {
    fn handle(&self) -> &EpHnd {
        &self.hnd
    }
}

/// saontehu
pub struct MemEpFactory;

impl AsEpFactory for MemEpFactory {
    fn bind(
        &self,
        _bind_spec: TxUrl,
        _timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<Ep>> {
        async move {
            Ok(MemEp::new())
        }.boxed()
    }
}
*/

/*
use crate::tx2::tx2_backend::*;
use crate::tx2::util::{Active, TxUrl};
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
            );
            let oth_con: Arc<dyn ConAdapt> = Arc::new(oth_con);

            let con = MemConAdapt::new(format!("{}/{}", url, con_id).into(), send, con_active);
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
            let rc: Box<dyn ConRecvAdapt> = Box::new(MemConRecvAdapt::new(c_recv, ep_active));
            Ok((ep, rc))
        }
        .boxed()
    }
}
*/

#[cfg(test)]
mod tests {
    //use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_tx2_mem2() {
        /*
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

        ep1.close().await;
        ep2.close().await;

        rt.await.unwrap();
        */
    }
}
