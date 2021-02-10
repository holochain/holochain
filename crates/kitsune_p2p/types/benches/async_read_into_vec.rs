use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kitsune_p2p_types::tx2::{AsyncReadIntoVec, AsyncReadIntoVecFilter};
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

const SIZE: usize = 1024 * 1024 * 8;

struct FakeRead;

impl futures::io::AsyncRead for FakeRead {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<Result<usize, futures::io::Error>> {
        static DATA: &'static [u8; 4096] = &[0xdb; 4096];
        let mut offset = 0;
        while offset < buf.len() {
            let len = std::cmp::min(4096, buf.len() - offset);
            buf[offset..offset + len].copy_from_slice(&DATA[0..len]);
            offset += len;
        }
        std::task::Poll::Ready(Ok(buf.len()))
    }
}

static VEC: Lazy<tokio::sync::Mutex<Option<Vec<u8>>>> =
    Lazy::new(|| tokio::sync::Mutex::new(Some(Vec::with_capacity(SIZE))));

fn async_read_into_vec(_black_box: ()) {
    futures::executor::block_on(RUNTIME.enter(|| {
        tokio::task::spawn(async move {
            let mut vec = VEC.lock().await.take().unwrap();
            vec.clear();

            let mut r = AsyncReadIntoVecFilter::new(Box::new(FakeRead));

            r.read_into_vec(&mut vec, SIZE).await.unwrap();
            assert_eq!(vec.len(), SIZE);

            (*VEC.lock().await) = Some(vec);
        })
    }))
    .unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("async_read_into_vec", |b| {
        b.iter(|| async_read_into_vec(black_box(())))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
