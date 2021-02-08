use std::sync::Arc;

use bencher::{benchmark_group, benchmark_main, Bencher};
use futures::StreamExt;
use ghost_actor::dependencies::observability;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::dependencies::url2::Url2;
use kitsune_p2p_types::transport::*;
// use observability::tracing::*;
use once_cell::sync::OnceCell;
use tokio::runtime::{Builder, Runtime};

benchmark_group!(
    benches,
    large_data_1_stream,
    large_data_10_streams,
    small_data_1_stream,
    small_data_100_streams
);
benchmark_main!(benches);

fn large_data_1_stream(bench: &mut Bencher) {
    send_data(bench, LARGE_DATA, 1);
}

fn large_data_10_streams(bench: &mut Bencher) {
    send_data(bench, LARGE_DATA, 10);
}

fn small_data_1_stream(bench: &mut Bencher) {
    send_data(bench, SMALL_DATA, 1);
}

fn small_data_100_streams(bench: &mut Bencher) {
    send_data(bench, SMALL_DATA, 100);
}

fn send_data(bench: &mut Bencher, data: &'static [u8], concurrent_streams: usize) {
    let _g = observability::test_run().ok();

    let addr = spawn_server();
    let (client, runtime) = make_client();
    let client = Arc::new(client);
    let mut runtime = runtime.lock().unwrap();

    bench.bytes = ((data.len() * 2) as u64) * (concurrent_streams as u64);
    bench.iter(|| {
        let mut handles = Vec::new();

        for _ in 0..concurrent_streams {
            let addr = addr.clone();
            let client = client.clone();
            handles.push(runtime.spawn(async move {
                client.request(addr, data.to_vec()).await.unwrap();
            }));
        }

        runtime.block_on(async {
            for handle in handles {
                handle.await.unwrap();
            }
        });
    });
}

fn spawn_server() -> Url2 {
    static INSTANCE: OnceCell<Url2> = OnceCell::new();
    INSTANCE.get_or_init(|| spawn_server_inner().0).clone()
}

fn spawn_server_inner() -> (Url2, std::thread::JoinHandle<()>) {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let handle = std::thread::spawn(move || {
        let mut runtime = rt();
        let handle = runtime.spawn(async move {
            let (listener, mut events2) =
                spawn_transport_listener_quic(ConfigListenerQuic::default())
                    .await
                    .unwrap();
            let bound = listener.bound_url().await.unwrap();
            tx.send(bound).unwrap();
            while let Some(evt) = events2.next().await {
                match evt {
                    TransportEvent::IncomingChannel(_url, mut write, read) => {
                        let data = read.read_to_end().await;
                        write.write_and_close(data).await.unwrap();
                    }
                }
            }
        });
        runtime.block_on(handle).unwrap();
    });
    let addr = rx.recv().unwrap();
    (addr, handle)
}

fn make_client() -> (
    ghost_actor::GhostSender<TransportListener>,
    &'static std::sync::Mutex<Runtime>,
) {
    static INSTANCE: OnceCell<(
        ghost_actor::GhostSender<TransportListener>,
        std::sync::Mutex<Runtime>,
    )> = OnceCell::new();
    let (l, r) = INSTANCE.get_or_init(|| {
        let ((l, _e), r) = make_client_inner();
        let r = std::sync::Mutex::new(r);
        (l, r)
    });
    (l.clone(), r)
}

fn make_client_inner() -> (
    (
        ghost_actor::GhostSender<TransportListener>,
        TransportEventReceiver,
    ),
    Runtime,
) {
    let mut runtime = rt();
    let client = runtime.block_on(async {
        let client = spawn_transport_listener_quic(
            ConfigListenerQuic::default().set_override_host(Some("127.0.0.1")),
        )
        .await
        .unwrap();

        let bound = client.0.bound_url().await.unwrap();
        assert_eq!("127.0.0.1", bound.host_str().unwrap());
        println!("listener1 bound to: {}", bound);
        client
    });
    (client, runtime)
}

fn rt() -> Runtime {
    Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap()
}

const LARGE_DATA: &[u8] = &[0xAB; 1024 * 1024];

const SMALL_DATA: &[u8] = &[0xAB; 1];
