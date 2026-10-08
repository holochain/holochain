//! Criterion benchmarks for the hot DHT database read paths.
//!
//! Run with `make bench-data` or
//! `cargo bench -p holochain_data --bench dht_queries`. Set
//! `HC_DATA_BENCH_SIZES=1000,10000` to limit fixture sizes while iterating
//! and `HC_DATA_BENCH_EXTRA_SQL=<file>` to apply extra statements (for
//! example candidate indexes) after the fixture is built.

pub mod fixture;

use criterion::{criterion_group, criterion_main, Criterion};
use std::sync::OnceLock;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

/// All fixtures, built once per process in ascending size order.
fn fixtures() -> &'static [fixture::Fixture] {
    static FX: OnceLock<Vec<fixture::Fixture>> = OnceLock::new();
    FX.get_or_init(|| {
        runtime().block_on(async {
            let mut out = Vec::new();
            for actions in fixture::sizes() {
                let started = std::time::Instant::now();
                let fx = fixture::build(fixture::FixtureConfig { actions, seed: 42 }).await;
                eprintln!(
                    "fixture {actions}: {} limbo rows, built in {:?}",
                    fx.keys.limbo_rows,
                    started.elapsed()
                );
                out.push(fx);
            }
            out
        })
    })
}

fn smoke(c: &mut Criterion) {
    let rt = runtime();
    for fx in fixtures() {
        let db = fx.db.as_ref();
        let author = fx.keys.local_author.clone();
        c.bench_function(&format!("smoke/chain_head/{}", fx.size), |b| {
            b.to_async(rt)
                .iter(|| async { db.chain_head_for_author(&author).await.unwrap() })
        });
    }
}

criterion_group!(benches, smoke);
criterion_main!(benches);
