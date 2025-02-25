use std::sync::Arc;

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;

use ::fixt::prelude::*;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::dependencies::url2::url2;
use kitsune_p2p::dht::arq::ArqSize;
use kitsune_p2p::fixt::*;
use kitsune_p2p::KitsuneSpace;
use kitsune_p2p_bootstrap::error::BootstrapClientError;
use kitsune_p2p_bootstrap::error::BootstrapClientResult;
use kitsune_p2p_types::bootstrap::RandomLimit;
use kitsune_p2p_types::bootstrap::RandomQuery;
use kitsune_p2p_types::fixt::UrlListFixturator;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;

criterion_group!(benches, bootstrap);

criterion_main!(benches);

fn bootstrap(bench: &mut Criterion) {
    let mut group = bench.benchmark_group("bootstrap");
    group.sample_size(
        std::env::var_os("BENCH_SAMPLE_SIZE")
            .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
            .unwrap_or(100),
    );
    let runtime = rt();
    let client = reqwest::Client::builder().use_rustls_tls().build().unwrap();

    let mut url = url2!("http://127.0.0.1:0");
    let (driver, addr, _shutdown) = runtime.block_on(async {
        kitsune_p2p_bootstrap::run(([127, 0, 0, 1], 0), vec![])
            .await
            .unwrap()
    });
    runtime.spawn(async move {
        driver.await;
        println!("BOOTSTRAP CLOSED");
    });
    url.set_port(Some(addr.port())).unwrap();
    group.bench_function(BenchmarkId::new("test", "now".to_string()), |b| {
        b.iter(|| {
            runtime.block_on(async {
                let time: u64 = do_api(url.clone(), "now", (), &client)
                    .await
                    .unwrap()
                    .unwrap();
                assert!(time > 0);
            });
        });
    });
    let space: Arc<KitsuneSpace> = runtime.block_on(async { Arc::new(fixt!(KitsuneSpace)) });
    group.bench_function(BenchmarkId::new("test", "put".to_string()), |b| {
        b.iter(|| {
            runtime.block_on(async {
                let info = AgentInfoSigned::sign(
                    space.clone(),
                    Arc::new(fixt!(KitsuneAgent, Unpredictable)),
                    ArqSize::from_half_len(u32::MAX / 4),
                    fixt!(UrlList, Empty),
                    0,
                    std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64 + 60_000_000,
                    |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Unpredictable))) },
                )
                .await
                .unwrap();
                let _: Option<()> = do_api(url.clone(), "put", info, &client)
                    .await
                    .unwrap()
                    .unwrap();
            });
        });
    });
    let query = RandomQuery {
        space,
        limit: RandomLimit(10),
    };
    group.bench_function(BenchmarkId::new("test", "random".to_string()), |b| {
        b.iter(|| {
            runtime.block_on(async {
                let peers: Vec<serde_bytes::ByteBuf> =
                    do_api(url.clone(), "random", query.clone(), &client)
                        .await
                        .unwrap()
                        .unwrap();
                assert_eq!(peers.len(), 10);
            });
        });
    });
    runtime.shutdown_background();
}

async fn do_api<I: serde::Serialize, O: serde::de::DeserializeOwned>(
    url: kitsune_p2p::dependencies::url2::Url2,
    op: &str,
    input: I,
    client: &reqwest::Client,
) -> BootstrapClientResult<Option<O>> {
    let mut body_data = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut body_data, &input)?;
    let res = client
        .post(url.as_str())
        .body(body_data)
        .header("X-Op", op)
        .header(reqwest::header::CONTENT_TYPE, "application/octet")
        .send()
        .await?;
    if res.status().is_success() {
        Ok(Some(kitsune_p2p_types::codec::rmp_decode(
            &mut res.bytes().await?.as_ref(),
        )?))
    } else {
        Err(BootstrapClientError::Bootstrap(
            res.text().await?.into_boxed_str(),
        ))
    }
}

pub fn rt() -> Runtime {
    Builder::new_multi_thread().enable_all().build().unwrap()
}
