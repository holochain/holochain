use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kitsune_p2p_types::tx2::AsyncOwnedResourceBucket;
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

static BUCKET: Lazy<AsyncOwnedResourceBucket<&'static str>> =
    Lazy::new(|| AsyncOwnedResourceBucket::new(None));

fn async_bucket(_black_box: ()) {
    futures::executor::block_on(RUNTIME.enter(|| {
        tokio::task::spawn(async move {
            let mut all = Vec::new();
            for _ in 0..100 {
                all.push(tokio::task::spawn(async move {
                    for _ in 0..100 {
                        let res = BUCKET.acquire().await.unwrap();
                        assert!(res == "1" || res == "2");
                        BUCKET.release(res).await;
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
    c.bench_function("async_bucket", |b| b.iter(|| async_bucket(black_box(()))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
