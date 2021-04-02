use criterion::{/*black_box,*/ criterion_group, criterion_main, Criterion};

use futures::stream::StreamExt;
use kitsune_p2p_proxy::*;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::dependencies::{ghost_actor, url2::Url2};
use kitsune_p2p_types::transport::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::sync::Arc;

const SIZE: usize = 2048;
const REQ: &[u8] = &[0xda; SIZE];
const RES: &[u8] = &[0xdb; SIZE];
const TGT_COUNT: usize = 10;
const NODE_COUNT: usize = 100;

async fn connect(
    proxy_config: Arc<ProxyConfig>,
) -> TransportResult<(Url2, ghost_actor::GhostSender<TransportListener>)> {
    //let (bind, evt) = kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?;
    let (bind, evt) = spawn_transport_listener_quic(Default::default()).await?;

    let (bind, mut evt) = spawn_kitsune_proxy_listener(
        proxy_config,
        Arc::new(KitsuneP2pTuningParams::default()),
        bind,
        evt,
    )
    .await?;
    let addr = bind.bound_url().await?;

    tokio::task::spawn(async move {
        while let Some(evt) = evt.next().await {
            match evt {
                TransportEvent::IncomingChannel(_url, mut write, read) => {
                    let data = read.read_to_end().await;
                    assert_eq!(REQ, data.as_slice());
                    write.write_and_close(RES.to_vec()).await?;
                }
            }
        }
        TransportResult::Ok(())
    });

    Ok((addr, bind))
}

#[allow(dead_code)]
struct Test {
    pub proxy: ghost_actor::GhostSender<TransportListener>,
    pub tgt_nodes: Vec<ghost_actor::GhostSender<TransportListener>>,
    pub tgt_addrs: Vec<Url2>,
    pub nodes: Vec<ghost_actor::GhostSender<TransportListener>>,
}

impl Test {
    pub async fn new() -> Self {
        let proxy_config = ProxyConfig::local_proxy_server(
            TlsConfig::new_ephemeral().await.unwrap(),
            AcceptProxyCallback::accept_all(),
        );
        let (proxy_addr, proxy) = connect(proxy_config).await.unwrap();

        let mut tgt_nodes = Vec::new();
        let mut tgt_addrs = Vec::new();
        for _ in 0..TGT_COUNT {
            let proxy_config = ProxyConfig::remote_proxy_client(
                TlsConfig::new_ephemeral().await.unwrap(),
                proxy_addr.clone().into(),
            );
            let (tgt_addr, tgt) = connect(proxy_config).await.unwrap();
            tgt_nodes.push(tgt);
            tgt_addrs.push(tgt_addr);
        }

        let mut nodes = Vec::new();
        for _ in 0..NODE_COUNT {
            let proxy_config = ProxyConfig::remote_proxy_client(
                TlsConfig::new_ephemeral().await.unwrap(),
                proxy_addr.clone().into(),
            );
            let (_addr, node) = connect(proxy_config).await.unwrap();
            nodes.push(node);
        }

        Self {
            proxy,
            tgt_nodes,
            tgt_addrs,
            nodes,
        }
    }

    pub async fn test(&mut self) {
        let tgts = self.tgt_addrs.clone();
        let mut tgt_iter = tgts.iter();

        let mut futs = Vec::new();
        for con in self.nodes.iter() {
            let tgt_addr = match tgt_iter.next() {
                Some(t) => t,
                None => {
                    tgt_iter = tgts.iter();
                    tgt_iter.next().unwrap()
                }
            };

            let chan_fut = con.create_channel(tgt_addr.clone());

            futs.push(async move {
                let (_, mut write, read) = chan_fut.await.unwrap();
                write.write_and_close(REQ.to_vec()).await?;
                let data = read.read_to_end().await;
                assert_eq!(RES, data.as_slice());
                TransportResult::Ok(())
            });
        }

        futures::future::try_join_all(futs).await.unwrap();
    }
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

fn old_proxy_thru(rt: &tokio::runtime::Runtime, t: &Share<Option<Test>>) {
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

    c.bench_function("old_proxy_thru", |b| b.iter(|| old_proxy_thru(&rt, &t)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
