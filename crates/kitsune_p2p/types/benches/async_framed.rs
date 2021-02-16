use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kitsune_p2p_types::{tx2::*, KitsuneTimeout};
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
const DATA: &[u8] = &[0xdb; SIZE];

static T: Lazy<tokio::sync::Mutex<Option<(AsyncWriteFramedFilter, AsyncReadFramedFilter)>>> =
    Lazy::new(|| {
        let (send, recv) = util::bound_async_mem_channel(4096);
        let send = AsyncWriteFramedFilter::new(send);
        let recv = Box::new(AsyncReadIntoVecFilter::new(recv));
        let recv = AsyncReadFramedFilter::new(recv);
        tokio::sync::Mutex::new(Some((send, recv)))
    });

fn async_framed() {
    futures::executor::block_on(RUNTIME.enter(|| {
        tokio::task::spawn(async move {
            let (mut send, mut recv) = T.lock().await.take().unwrap();

            let send_t = tokio::task::spawn(async move {
                send.write_frame(
                    0.into(),
                    black_box(DATA),
                    KitsuneTimeout::from_millis(1000 * 30),
                )
                .await
                .unwrap();
                send
            });

            'top: loop {
                let mut frames = Some(Vec::new());
                recv.read_frame(
                    KitsuneTimeout::from_millis(1000 * 30),
                    black_box(&mut frames),
                )
                .await
                .unwrap();
                for (msg_id, frame) in frames.as_mut().unwrap().drain(..) {
                    assert_eq!(0, msg_id.as_id());
                    assert_eq!(SIZE, frame.len());
                    break 'top;
                }
            }

            let send = send_t.await.unwrap();

            (*T.lock().await) = Some((send, recv));
        })
    }))
    .unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("async_framed", |b| b.iter(|| async_framed()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
