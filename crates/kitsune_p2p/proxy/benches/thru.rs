use criterion::{/*black_box,*/ criterion_group, criterion_main, Criterion};
use futures::stream::StreamExt;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_proxy::*;
use kitsune_p2p_types::tx2::tx2_frontend::*;
use kitsune_p2p_types::tx2::tx2_promote::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use once_cell::sync::Lazy;
use std::sync::Arc;

static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

const SIZE: usize = 2048;
const REQ: &[u8] = &[0xda; SIZE];
const RES: &[u8] = &[0xdb; SIZE];
const TGT_COUNT: usize = 10;
const NODE_COUNT: usize = 100;

struct Test {
    pub proxy_ep_hnd: EpHnd,
    pub tgt_nodes: Vec<(EpHnd, ConHnd)>,
    pub tgt_addrs: Vec<TxUrl>,
    pub nodes: Vec<(EpHnd, ConHnd)>,

    pub d_send: Arc<Share<Option<tokio::sync::mpsc::Sender<()>>>>,
}

fn proxify_addr(purl: &TxUrl, nurl: &TxUrl) -> TxUrl {
    let digest = ProxyUrl::from(nurl.as_str());
    let digest = digest.digest();
    let purl = ProxyUrl::from(purl.as_str());
    ProxyUrl::new(purl.as_base().as_str(), digest)
        .unwrap()
        .as_str()
        .into()
}

impl Test {
    pub async fn new() -> Self {
        let t = KitsuneTimeout::from_millis(5000);

        let (proxy_addr, proxy_ep_hnd) = mk_proxy().await;

        let mut tgt_nodes = Vec::new();
        let mut tgt_addrs = Vec::new();
        for _ in 0..TGT_COUNT {
            let (tgt_addr, tgt_ep_hnd) = mk_tgt().await;
            let tgt_con = tgt_ep_hnd.connect(proxy_addr.clone(), t).await.unwrap();
            tgt_nodes.push((tgt_ep_hnd, tgt_con));

            tgt_addrs.push(proxify_addr(&proxy_addr, &tgt_addr));
        }

        let d_send = Arc::new(Share::new(None));

        let mut nodes = Vec::new();
        {
            let mut tgt_iter = tgt_addrs.iter();
            for _ in 0..NODE_COUNT {
                let tgt_addr = match tgt_iter.next() {
                    Some(t) => t,
                    None => {
                        tgt_iter = tgt_addrs.iter();
                        tgt_iter.next().unwrap()
                    }
                };
                let (_, ep_hnd) = mk_node(d_send.clone()).await;
                let con = ep_hnd.connect(tgt_addr.clone(), t).await.unwrap();
                nodes.push((ep_hnd, con));
            }
        }

        Self {
            proxy_ep_hnd,
            tgt_nodes,
            tgt_addrs,
            nodes,
            d_send,
        }
    }

    pub async fn test(&mut self) {
        let (d_send, mut d_recv) = tokio::sync::mpsc::channel(self.nodes.len());

        self.d_send
            .share_mut(move |i, _| {
                *i = Some(d_send);
                Ok(())
            })
            .unwrap();

        let mut futs = Vec::new();
        for (_, con) in self.nodes.iter() {
            futs.push(async move {
                let t = KitsuneTimeout::from_millis(5000);
                let mut data = PoolBuf::new();
                data.extend_from_slice(REQ);
                con.write(0.into(), data, t).await?;
                KitsuneResult::Ok(())
            });
        }

        futures::future::try_join_all(futs).await.unwrap();

        for _ in 0..self.nodes.len() {
            d_recv.recv().await;
        }

        self.d_send
            .share_mut(|i, _| {
                *i = None;
                Ok(())
            })
            .unwrap();
    }
}

async fn mk_core() -> (TxUrl, Ep, EpHnd) {
    let t = KitsuneTimeout::from_millis(5000);

    let f = tx2_promote(MemBackendAdapt::new(), NODE_COUNT * 3);
    let f = tx2_proxy(f, TlsConfig::new_ephemeral().await.unwrap());

    let ep = f.bind("none:", t).await.unwrap();
    let ep_hnd = ep.handle().clone();
    let addr = ep_hnd.local_addr().unwrap();

    (addr, ep, ep_hnd)
}

async fn mk_proxy() -> (TxUrl, EpHnd) {
    let (addr, mut ep, ep_hnd) = mk_core().await;

    tokio::task::spawn(async move { while let Some(_evt) = ep.next().await {} });

    (addr, ep_hnd)
}

async fn mk_tgt() -> (TxUrl, EpHnd) {
    let (addr, mut ep, ep_hnd) = mk_core().await;

    tokio::task::spawn(async move {
        while let Some(evt) = ep.next().await {
            if let EpEvent::IncomingData(EpIncomingData { con, mut data, .. }) = evt {
                let t = KitsuneTimeout::from_millis(5000);
                if data.as_ref() == REQ {
                    data.clear();
                    data.extend_from_slice(RES);
                    con.write(0.into(), data, t).await.unwrap();
                } else {
                    panic!("unexpected bytes");
                }
            }
        }
    });

    (addr, ep_hnd)
}

async fn mk_node(d_send: Arc<Share<Option<tokio::sync::mpsc::Sender<()>>>>) -> (TxUrl, EpHnd) {
    let (addr, mut ep, ep_hnd) = mk_core().await;

    tokio::task::spawn(async move {
        while let Some(evt) = ep.next().await {
            if let EpEvent::IncomingData(EpIncomingData { data, .. }) = evt {
                if data.as_ref() == RES {
                    let d_send = d_send
                        .share_mut(|i, _| Ok(i.as_ref().unwrap().clone()))
                        .unwrap();
                    d_send.send(()).await.unwrap();
                } else {
                    panic!("unexpected bytes");
                }
            }
        }
    });

    (addr, ep_hnd)
}

async fn test(this: &Arc<Share<Option<Test>>>) {
    let mut t = this.share_mut(|i, _| Ok(i.take().unwrap())).unwrap();
    t.test().await;
    this.share_mut(move |i, _| {
        *i = Some(t);
        Ok(())
    })
    .unwrap();
}

fn thru(t: &Arc<Share<Option<Test>>>) {
    let _g = RUNTIME.enter();
    RUNTIME.block_on(async {
        test(t).await;
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    let t = RUNTIME.block_on(async { Arc::new(Share::new(Some(Test::new().await))) });
    c.bench_function("thru", |b| b.iter(|| thru(&t)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
