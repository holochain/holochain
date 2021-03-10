use criterion::{/*black_box,*/ criterion_group, criterion_main, Criterion};
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use once_cell::sync::{Lazy, OnceCell};

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
static T: OnceCell<tokio::sync::Mutex<Option<(FramedWriter, FramedReader)>>> = OnceCell::new();

fn framed() {
    T.get_or_init(|| {
        let (send, recv) = bound_async_mem_channel(4096);
        tokio::sync::Mutex::new(Some((FramedWriter::new(send), FramedReader::new(recv))))
    });

    futures::executor::block_on(RUNTIME.enter(|| {
        tokio::task::spawn(async move {
            let (mut send, mut recv) = T.get().unwrap().lock().await.take().unwrap();

            let mut buf = PoolBuf::new();
            buf.extend_from_slice(DATA);

            let wt = tokio::task::spawn(async move {
                send.write(1.into(), buf, KitsuneTimeout::from_millis(1000 * 30))
                    .await
                    .unwrap();
                send
            });

            let (msg_id, data) = recv
                .read(KitsuneTimeout::from_millis(1000 * 30))
                .await
                .unwrap();

            assert_eq!(1, msg_id.as_id());
            assert_eq!(SIZE, data.len());

            let send = wt.await.unwrap();

            (*T.get().unwrap().lock().await) = Some((send, recv));
        })
    }))
    .unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("framed", |b| b.iter(|| framed()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
