use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kitsune_p2p_types::tx2::tx2_utils::*;
use once_cell::sync::Lazy;

static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

static BUCKET: Lazy<ResourceBucket<&'static str>> = Lazy::new(|| ResourceBucket::new());

fn resource_bucket() {
    let _g = RUNTIME.enter();

    RUNTIME.block_on(async move {
        let mut all = Vec::new();
        for _ in 0..50 {
            all.push(tokio::task::spawn(async move {
                for _ in 0..50 {
                    let res = BUCKET.acquire(None).await.unwrap();
                    assert!(res == "1" || res == "2");
                    BUCKET.release(black_box(res));
                }
            }));
        }
        futures::future::try_join_all(all).await.unwrap();
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    let _g = RUNTIME.enter();
    RUNTIME.block_on(async move {
        BUCKET.release("1");
        BUCKET.release("2");
    });
    c.bench_function("resource_bucket", |b| b.iter(|| resource_bucket()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
