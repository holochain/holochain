use std::sync::Arc;

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use futures::StreamExt;
use ghost_actor::dependencies::observability;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::dependencies::url2::Url2;
use kitsune_p2p_types::transport::*;
// use observability::tracing::*;
use once_cell::sync::OnceCell;
use tokio::runtime::{Builder, Runtime};

const DATA: &[u8] = &[0xAB; 100];

criterion_group!(benches, send_data,);

criterion_main!(benches);

fn send_data(bench: &mut Criterion) {
    let _g = observability::test_run().ok();

    let addr = spawn_server();
    let (client, runtime) = make_client();
    let client = Arc::new(client);
    let mut runtime = runtime.lock().unwrap();

    let mut group = bench.benchmark_group("send_data");
    for &(data, messages, series) in [
        (DATA, 1000, true),
        (DATA, 1000, false),
    ]
    .iter()
    {
        let bytes = ((data.len() * 2) as u64) * (messages as u64);
        group.throughput(Throughput::Bytes(bytes));
        group.sample_size(10);
        group.bench_with_input(
            BenchmarkId::new(
                "send_msg",
                format!(
                    "messages_{}_series_{}_bytes_{}",
                    messages,
                    series,
                    bytes,
                ),
            ),
            &(data, messages),
            |b, &(data, messages)| {
                b.iter(|| {
                    let mut handles = Vec::new();

                    let addr = addr.clone();
                    let client = client.clone();
                    if series {
                        handles.push(runtime.spawn(async move {
                            for _ in 0..messages {
                                let addr = addr.clone();
                                let client = client.clone();
                                client.request(addr, data.to_vec()).await.unwrap();
                            }
                        }));
                    } else {
                        for _ in 0..messages {
                            let addr = addr.clone();
                            let client = client.clone();
                            handles.push(runtime.spawn(async move {
                                client.request(addr, data.to_vec()).await.unwrap();
                            }));
                        }
                    }

                    runtime.block_on(async {
                        for handle in handles {
                            handle.await.unwrap();
                        }
                    });
                })
            },
        );
    }
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
