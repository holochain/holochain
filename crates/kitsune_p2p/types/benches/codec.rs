use criterion::{/*black_box,*/ criterion_group, criterion_main, Criterion};
use kitsune_p2p_types::{tx2::*, KitsuneTimeout};
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

kitsune_p2p_types::write_codec_enum! {
    codec Test {
        One(0x01) {
            data.0: PoolBuf,
        },
    }
}

const SIZE: usize = 1024;
const DATA: &[u8] = &[0xdb; SIZE];
static T: OnceCell<tokio::sync::Mutex<Option<(CodecWriter<Test>, CodecReader<Test>)>>> =
    OnceCell::new();

static NEXT_MESSAGE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn codec() {
    T.get_or_init(|| {
        let (send, recv) = util::bound_async_mem_channel(4096);
        tokio::sync::Mutex::new(Some((
            CodecWriter::new(FramedWriter::new(send)),
            CodecReader::new(FramedReader::new(recv)),
        )))
    });

    futures::executor::block_on(RUNTIME.enter(|| {
        tokio::task::spawn(async move {
            let (mut send, mut recv) = T.get().unwrap().lock().await.take().unwrap();

            let wt = tokio::task::spawn(async move {
                let mut buf = PoolBuf::new();
                buf.extend_from_slice(DATA);
                let msg = Test::one(buf);
                send.write_request(
                    NEXT_MESSAGE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    &msg,
                    KitsuneTimeout::from_millis(1000 * 30),
                )
                .await
                .unwrap();

                send
            });

            let mut got = 0;
            for _data in recv
                .read(KitsuneTimeout::from_millis(1000 * 30))
                .await
                .unwrap()
            {
                got += 1;
            }
            assert_eq!(1, got);

            let send = wt.await.unwrap();

            (*T.get().unwrap().lock().await) = Some((send, recv));
        })
    }))
    .unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("codec", |b| b.iter(|| codec()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
