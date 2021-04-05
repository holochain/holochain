#![allow(irrefutable_let_patterns)]
use criterion::{/*black_box,*/ criterion_group, criterion_main, Criterion};

use futures::stream::StreamExt;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;

const SIZE: usize = 2048;

kitsune_p2p_types::write_codec_enum! {
    codec TestData {
        One(0x01) {
            data.0: PoolBuf,
        },
    }
}

const REQ: &[u8] = &[0xda; SIZE];
const RES: &[u8] = &[0xdb; SIZE];

#[allow(dead_code)]
struct Test {
    dst_ep: Tx2EpHnd<TestData>,
    dst_url: TxUrl,
    src_ep: Tx2EpHnd<TestData>,
}

impl Test {
    pub async fn new() -> Self {
        let (dst_url, dst_ep) = mk_dst().await;
        let src_ep = mk_src().await;

        Self {
            dst_ep,
            dst_url,
            src_ep,
        }
    }

    pub async fn test(&mut self) {
        let t = KitsuneTimeout::from_millis(5000);

        let mut data = PoolBuf::new();
        data.extend_from_slice(REQ);
        let data = TestData::one(data);

        let con = self
            .src_ep
            .get_connection(self.dst_url.clone(), t)
            .await
            .unwrap();

        let res = con
            .request(&data, KitsuneTimeout::from_millis(5000))
            .await
            .unwrap();

        if let TestData::One(One { data }) = res {
            assert_eq!(data.as_ref(), RES);
        } else {
            panic!("invalid data");
        }
    }
}

async fn mk_core() -> (TxUrl, Tx2Ep<TestData>, Tx2EpHnd<TestData>) {
    let t = KitsuneTimeout::from_millis(5000);

    let f = tx2_mem_adapter(MemConfig::default()).await.unwrap();
    let f = tx2_pool_promote(f, 32);
    let f = tx2_api(f);

    let ep = f.bind("none:", t).await.unwrap();
    let ep_hnd = ep.handle().clone();
    let addr = ep_hnd.local_addr().unwrap();

    (addr, ep, ep_hnd)
}

async fn mk_dst() -> (TxUrl, Tx2EpHnd<TestData>) {
    let (url, mut ep, ep_hnd) = mk_core().await;

    tokio::task::spawn(async move {
        while let Some(evt) = ep.next().await {
            if let Tx2EpEvent::IncomingRequest(Tx2EpIncomingRequest { data, respond, .. }) = evt {
                if let TestData::One(One { data }) = data {
                    assert_eq!(data.as_ref(), REQ);
                } else {
                    panic!("invalid data");
                }
                let mut data = PoolBuf::new();
                data.extend_from_slice(RES);
                let data = TestData::one(data);
                respond
                    .respond(data, KitsuneTimeout::from_millis(5000))
                    .await
                    .unwrap();
            }
        }
    });

    (url, ep_hnd)
}

async fn mk_src() -> Tx2EpHnd<TestData> {
    let (_url, mut ep, ep_hnd) = mk_core().await;

    tokio::task::spawn(async move { while let Some(_evt) = ep.next().await {} });

    ep_hnd
}

async fn test(this: &Share<Option<Test>>) {
    let mut t = this.share_mut(|i, _| Ok(i.take().unwrap())).unwrap();
    t.test().await;
    this.share_mut(move |i, _| {
        *i = Some(t);
        Ok(())
    })
    .unwrap();
}

fn api_thru(rt: &tokio::runtime::Runtime, t: &Share<Option<Test>>) {
    rt.block_on(async {
        test(t).await;
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let t = rt.block_on(async { Share::new(Some(Test::new().await)) });

    c.bench_function("api-thru-mem", |b| b.iter(|| api_thru(&rt, &t)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
