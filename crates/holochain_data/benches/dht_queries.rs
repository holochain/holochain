//! Criterion benchmarks for the hot DHT database read paths.
//!
//! Run with `make bench-data` or
//! `cargo bench -p holochain_data --bench dht_queries`. Set
//! `HC_DATA_BENCH_SIZES=1000,10000` to limit fixture sizes while iterating
//! and `HC_DATA_BENCH_EXTRA_SQL=<file>` to apply extra statements (for
//! example candidate indexes) after the fixture is built.

mod fixture;

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

fn smoke(c: &mut Criterion) {
    let rt = runtime();
    c.bench_function("smoke/noop", |b| b.to_async(rt).iter(|| async {}));
}

criterion_group!(benches, smoke);
criterion_main!(benches);
