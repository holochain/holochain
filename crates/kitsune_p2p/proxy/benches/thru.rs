use criterion::{/*black_box,*/ criterion_group, criterion_main, Criterion};
use futures::stream::StreamExt;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_proxy::*;
use kitsune_p2p_types::tx2::tx2_frontend::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

const SIZE: usize = 2048;
const REQ: &[u8] = &[0xda; SIZE];
const RES: &[u8] = &[0xdb; SIZE];
const NODE_COUNT: usize = 80;

type SND = tokio::sync::mpsc::Sender<()>;
type RCV = tokio::sync::mpsc::Receiver<()>;
static CHAN: Lazy<(parking_lot::Mutex<SND>, parking_lot::Mutex<Option<RCV>>)> = Lazy::new(|| {
    let (s, r) = tokio::sync::mpsc::channel(NODE_COUNT);
    (parking_lot::Mutex::new(s), parking_lot::Mutex::new(Some(r)))
});

static REQ_CNT: AtomicUsize = AtomicUsize::new(0);
static RES_CNT: AtomicUsize = AtomicUsize::new(0);

async fn build_node() -> (TxUrl, EpHnd) {
    let t = KitsuneTimeout::from_millis(5000);

    let f = tx2_proxy(
        MemBackendAdapt::new(),
        TlsConfig::new_ephemeral().await.unwrap(),
        NODE_COUNT + 10,
    );
    let mut ep = f.bind("none:", t).await.unwrap();
    let ephnd = ep.handle().clone();
    let addr = ephnd.local_addr().unwrap();

    tokio::task::spawn(async move {
        while let Some(evt) = ep.next().await {
            if let EpEvent::IncomingData(con, _, mut data) = evt {
                if data.as_ref() == REQ {
                    data.clear();
                    data.extend_from_slice(RES);
                    let t = KitsuneTimeout::from_millis(5000);
                    con.write(0.into(), data, t).await.unwrap();

                    let c = REQ_CNT.fetch_add(1, Ordering::Relaxed);
                    if c % 1000 == 0 {
                        println!("req_count: {}", c);
                    }
                } else if data.as_ref() == RES {
                    let s = CHAN.0.lock().clone();
                    let _ = s.send(()).await.unwrap();

                    let c = RES_CNT.fetch_add(1, Ordering::Relaxed);
                    if c % 1000 == 0 {
                        println!("res_count: {}", c);
                    }
                } else {
                    panic!(
                        "invalid data received {} bytes starting with {:02x?}..",
                        data.len(),
                        &data[..4]
                    );
                }
            }
        }
    });

    (addr, ephnd)
}

static PROXY: Lazy<parking_lot::Mutex<(TxUrl, EpHnd)>> = Lazy::new(|| {
    let _g = RUNTIME.enter();
    RUNTIME.block_on(async move { parking_lot::Mutex::new(build_node().await) })
});

fn proxify_addr(purl: &TxUrl, nurl: &TxUrl) -> TxUrl {
    let digest = ProxyUrl::from(nurl.as_str());
    let digest = digest.digest();
    let purl = ProxyUrl::from(purl.as_str());
    ProxyUrl::new(purl.as_base().as_str(), digest)
        .unwrap()
        .as_str()
        .into()
}

static TARGET: Lazy<parking_lot::Mutex<(TxUrl, EpHnd, ConHnd)>> = Lazy::new(|| {
    let _g = RUNTIME.enter();
    RUNTIME.block_on(async move {
        let t = KitsuneTimeout::from_millis(5000);
        let proxy_url = PROXY.lock().0.clone();
        //println!("proxy_url: {}", proxy_url);
        let (t_url, ep) = build_node().await;
        //println!("t_url: {}", t_url);
        let con = ep.connect(proxy_url.clone(), t).await.unwrap();
        let pt_url = proxify_addr(&proxy_url, &t_url);
        //println!("pt_url: {}", pt_url);
        parking_lot::Mutex::new((pt_url, ep, con))
    })
});

static N: Lazy<parking_lot::Mutex<Vec<(EpHnd, ConHnd)>>> = Lazy::new(|| {
    let _g = RUNTIME.enter();
    RUNTIME.block_on(async move {
        let t = KitsuneTimeout::from_millis(5000);
        let pt_url = TARGET.lock().0.clone();
        let mut out = Vec::with_capacity(NODE_COUNT);
        for _ in 0..NODE_COUNT {
            let (_, ep) = build_node().await;
            let con = ep.connect(pt_url.clone(), t).await.unwrap();
            out.push((ep, con));
        }
        parking_lot::Mutex::new(out)
    })
});

static TCNT: AtomicUsize = AtomicUsize::new(0);

fn thru() {
    let _g = RUNTIME.enter();
    RUNTIME.block_on(async move {
        let t = KitsuneTimeout::from_millis(5000);
        let mut all = Vec::with_capacity(NODE_COUNT);
        for (_, c) in N.lock().iter() {
            let mut data = PoolBuf::new();
            data.extend_from_slice(REQ);
            all.push(c.write(0.into(), data, t));
        }
        futures::future::try_join_all(all).await.unwrap();
        let mut r = CHAN.1.lock().take().unwrap();
        for _ in 0..NODE_COUNT {
            let _ = r.recv().await;
        }
        (*CHAN.1.lock()) = Some(r);
        let c = TCNT.fetch_add(1, Ordering::Relaxed);
        if c % 100 == 0 {
            println!("total_count: {}", c);
        }
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    // these must be initialized outside the bench fn
    &*CHAN;
    &*PROXY;
    &*TARGET;
    &*N;

    c.bench_function("thru", |b| b.iter(|| thru()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
