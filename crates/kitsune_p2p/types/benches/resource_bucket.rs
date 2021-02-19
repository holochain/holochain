use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kitsune_p2p_types::tx2::ResourceBucket;
use once_cell::sync::Lazy;

static RUNTIME: Lazy<tokio::runtime::Handle> = Lazy::new(|| {
    let mut rt = tokio::runtime::Builder::new()
        .enable_all()
        .threaded_scheduler()
        .build()
        .unwrap();
    let handle = rt.handle().clone();
    std::thread::spawn(move || {
        rt.block_on(futures::future::pending::<()>());
    });
    handle
});

static BUCKET: Lazy<ResourceBucket<&'static str>> = Lazy::new(|| ResourceBucket::new(None));

fn resource_bucket() {
    futures::executor::block_on(RUNTIME.enter(|| {
        tokio::task::spawn(async move {
            let mut all = Vec::new();
            for _ in 0..100 {
                all.push(tokio::task::spawn(async move {
                    for _ in 0..100 {
                        let res = BUCKET.acquire().await.unwrap();
                        assert!(res == "1" || res == "2");
                        BUCKET.release(black_box(res)).await;
                    }
                }));
            }
            futures::future::try_join_all(all).await.unwrap();
        })
    }))
    .unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    futures::executor::block_on(async move {
        BUCKET.release("1").await;
        BUCKET.release("2").await;
    });
    c.bench_function("resource_bucket", |b| b.iter(|| resource_bucket()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
