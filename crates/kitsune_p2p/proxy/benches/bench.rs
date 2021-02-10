use std::sync::Arc;

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use futures::StreamExt;
use ghost_actor::dependencies::observability;
use kitsune_p2p_proxy::*;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::dependencies::url2::Url2;
use kitsune_p2p_types::{transport::*, transport_mem::*};
use observability::tracing::*;
use once_cell::sync::OnceCell;
use tokio::runtime::{Builder, Runtime};

const DATA: &[u8] = &[0xAB; 100];
const LARGE_DATA: &[u8] = &[0xAB; 1024 * 10];

const NUM_RECV_CONCURRENT: usize = 100;

// const PROCESS_DELAY_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

criterion_group!(benches, request, to_proxy, channels, mem, send_recv, send);

criterion_main!(benches);

fn to_proxy(bench: &mut Criterion) {
    let _g = observability::test_run().ok();

    let proxy_url = spawn_proxy_quic();
    let (client, runtime) = make_client_quic(proxy_url.clone());
    let client = Arc::new(client);
    let mut runtime = runtime.lock().unwrap();

    let mut group = bench.benchmark_group("to_proxy");
    for &(data, messages, series) in [(DATA, 1000, true)].iter() {
        let bytes = ((data.len() * 2) as u64) * (messages as u64);
        group.throughput(Throughput::Bytes(bytes));
        group.sample_size(10);
        group.bench_with_input(
            BenchmarkId::new(
                "send_msgs",
                format!("messages_{}_series_{}_bytes_{}", messages, series, bytes,),
            ),
            &(data, messages),
            |b, &(data, messages)| {
                b.iter(|| {
                    let mut handles = Vec::new();

                    let proxy_url = proxy_url.clone();
                    let client = client.clone();
                    if series {
                        handles.push(runtime.spawn(async move {
                            for _ in 0..messages {
                                let proxy_url = proxy_url.clone();
                                let client = client.clone();
                                let (_, mut write, read) =
                                    client.create_channel(proxy_url.clone()).await.unwrap();
                                write.write_and_close(data.to_vec()).await.unwrap();
                                read.read_to_end().await;
                            }
                        }));
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

fn request(bench: &mut Criterion) {
    let _g = observability::test_run().ok();

    let proxy_url = spawn_proxy_quic();
    let responder_url = spawn_responder_quic(proxy_url.clone());
    let (client, runtime) = make_client_quic(proxy_url.clone());
    let client = Arc::new(client);
    let mut runtime = runtime.lock().unwrap();

    let mut group = bench.benchmark_group("request");
    for &(data, messages, series, do_channel, do_write, do_read) in [
        (DATA, 1000, true, true, true, true),
        (LARGE_DATA, 1000, true, true, true, true),
        (DATA, 1000, false, true, true, true),
        (DATA, 1000, true, false, false, false),
        (DATA, 1000, true, true, false, false),
        (DATA, 1000, true, true, true, false),
    ]
    .iter()
    {
        let bytes = ((data.len() * 2) as u64) * (messages as u64);
        group.throughput(Throughput::Bytes(bytes));
        group.sample_size(10);
        group.bench_with_input(
            BenchmarkId::new(
                "baseline",
                format!(
                    "messages_{}_series_{}_bytes_{}_chan_{}_write_{}_read_{}",
                    messages, series, bytes, do_channel, do_write, do_read
                ),
            ),
            &(data, messages),
            |b, &(_data, messages)| {
                b.iter(|| {
                    let mut handles = Vec::new();

                    let client = client.clone();
                    let test = {
                        let responder_url = responder_url.clone();
                        let client = client.clone();
                        let data = data.to_vec();
                        let do_channel = do_channel;
                        let do_write = do_write;
                        let do_read = do_read;
                        move || {
                            let responder_url = responder_url.clone();
                            let client = client.clone();
                            let data = data.to_vec();
                            let do_channel = do_channel;
                            let do_write = do_write;
                            let do_read = do_read;
                            async move {
                                if do_channel {
                                    let (_, mut write, read) =
                                        client.create_channel(responder_url.clone()).await.unwrap();
                                    if do_write {
                                        write.write_and_close(data).await.unwrap();
                                        if do_read {
                                            read.read_to_end().await;
                                        }
                                    }
                                }
                            }
                        }
                    };
                    if series {
                        handles.push(runtime.spawn(async move {
                            for _ in 0..messages {
                                test().await;
                            }
                        }));
                    } else {
                        for _ in 0..messages {
                            let f = test();
                            handles.push(runtime.spawn(f));
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

fn mem(bench: &mut Criterion) {
    let _g = observability::test_run().ok();

    let proxy_url = spawn_proxy_mem();
    let responder_url = spawn_responder_mem(proxy_url.clone());
    let (client, runtime) = make_client_mem(proxy_url.clone());
    let client = Arc::new(client);
    let mut runtime = runtime.lock().unwrap();

    let mut group = bench.benchmark_group("mem");
    for &(data, messages) in [(DATA, 1000), (LARGE_DATA, 1000)].iter() {
        let bytes = ((data.len() * 2) as u64) * (messages as u64);
        group.throughput(Throughput::Bytes(bytes));
        group.sample_size(10);
        group.bench_with_input(
            BenchmarkId::new(
                "mem_baseline",
                format!("messages_{}_bytes_{}", messages, bytes,),
            ),
            &(data, messages),
            |b, &(_data, messages)| {
                b.iter(|| {
                    let mut handles = Vec::new();

                    let responder_url = responder_url.clone();
                    let client = client.clone();
                    handles.push(runtime.spawn(async move {
                        for _ in 0..messages {
                            let responder_url = responder_url.clone();
                            let client = client.clone();
                            let (_, mut write, read) =
                                client.create_channel(responder_url.clone()).await.unwrap();
                            write.write_and_close(data.to_vec()).await.unwrap();
                            read.read_to_end().await;
                        }
                    }));

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

fn channels(bench: &mut Criterion) {
    let _g = observability::test_run().ok();

    let proxy_url = spawn_proxy_quic();
    let (_client, runtime) = make_client_quic(proxy_url.clone());
    let mut runtime = runtime.lock().unwrap();

    let mut group = bench.benchmark_group("channels");
    static ONE: &'static [u8] = &[0u8];
    for &(data, messages) in [(DATA, 1000), (ONE, 1000), (LARGE_DATA, 1000)].iter() {
        let bytes = (data.len() as u64) * (messages as u64);
        group.throughput(Throughput::Bytes(bytes));
        group.sample_size(100);
        group.bench_with_input(
            BenchmarkId::new(
                "channel_speed",
                format!("messages_{}_bytes_{}", messages, bytes,),
            ),
            &(data, messages),
            |b, &(data, messages)| {
                b.iter(|| {
                    let mut handles = Vec::new();

                    use futures::SinkExt;
                    handles.push(runtime.spawn(async move {
                        for _ in 0..messages {
                            let (tx, mut rx) = futures::channel::mpsc::channel(10);
                            let mut tx =
                                Box::new(tx.sink_map_err(|e| TransportError::Other(e.into())));
                            tx.send(data).await.unwrap();
                            tx.close().await.unwrap();
                            rx.next().await.unwrap();
                        }
                    }));

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

fn send_recv(bench: &mut Criterion) {
    let _g = observability::test_run().ok();

    let proxy_url = spawn_proxy_quic();
    let (_client, runtime) = make_client_quic(proxy_url.clone());
    let mut runtime = runtime.lock().unwrap();

    let mut group = bench.benchmark_group("send_recv");
    static ONE: &'static [u8] = &[0u8];
    for &(data, messages) in [(DATA, 1000), (ONE, 1000), (LARGE_DATA, 1000)].iter() {
        let bytes = (data.len() as u64) * (messages as u64);
        group.throughput(Throughput::Bytes(bytes));
        group.sample_size(100);
        use futures::SinkExt;
        group.bench_with_input(
            BenchmarkId::new(
                "channel_speed",
                format!("messages_{}_bytes_{}", messages, bytes,),
            ),
            &(data, messages),
            |b, &(data, messages)| {
                b.iter(|| {
                    let mut handles = Vec::new();

                    handles.push(runtime.spawn(async move {
                        let (tx, mut rx) = futures::channel::mpsc::channel(messages);
                        let mut jhs = Vec::new();
                        for _ in 0..messages {
                            let jh = tokio::spawn({
                                let tx = tx.clone();
                                async move {
                                    let mut tx = Box::new(
                                        tx.sink_map_err(|e| TransportError::Other(e.into())),
                                    );
                                    tx.send(data).await.unwrap();
                                    tx
                                }
                            });
                            jhs.push(jh);
                        }

                        for _ in 0..messages {
                            rx.next().await.unwrap();
                        }
                    }));

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

fn send(bench: &mut Criterion) {
    let _g = observability::test_run().ok();

    let proxy_url = spawn_proxy_quic();
    let (_client, runtime) = make_client_quic(proxy_url.clone());
    let mut runtime = runtime.lock().unwrap();

    let mut group = bench.benchmark_group("send");
    static ONE: &'static [u8] = &[0u8];
    for &(data, messages, create_fresh) in [
        (DATA, 1000, true),
        (ONE, 1000, true),
        (LARGE_DATA, 1000, true),
        (DATA, 1000, false),
        (ONE, 1000, false),
        (LARGE_DATA, 1000, false),
    ]
    .iter()
    {
        let bytes = (data.len() as u64) * (messages as u64);
        group.throughput(Throughput::Bytes(bytes));
        group.sample_size(100);
        use futures::SinkExt;
        group.bench_with_input(
            BenchmarkId::new(
                "channel_speed",
                format!(
                    "messages_{}_create_{}_bytes_{}",
                    messages, create_fresh, bytes,
                ),
            ),
            &(data, messages),
            |b, &(data, messages)| {
                b.iter(|| {
                    let mut handles = Vec::new();

                    handles.push(runtime.spawn(async move {
                        if create_fresh {
                            for _ in 0..messages {
                                let (tx, _rx) = futures::channel::mpsc::channel(10);
                                let mut tx =
                                    Box::new(tx.sink_map_err(|e| TransportError::Other(e.into())));
                                tx.send(data).await.unwrap();
                                // tx.close().await.unwrap();
                            }
                        } else {
                            let (tx, _rx) = futures::channel::mpsc::channel(messages);
                            let mut tx =
                                Box::new(tx.sink_map_err(|e| TransportError::Other(e.into())));
                            for _ in 0..messages {
                                let mut tx = tx.clone();
                                tx.send(data).await.unwrap();
                            }
                            tx.close().await.unwrap();
                        }
                    }));

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
fn spawn_responder_quic(proxy_url: Url2) -> Url2 {
    static INSTANCE: OnceCell<Url2> = OnceCell::new();
    INSTANCE
        .get_or_init(|| spawn_responder_inner(ProxyTransport::Quic, proxy_url))
        .clone()
}

fn spawn_responder_mem(proxy_url: Url2) -> Url2 {
    static INSTANCE: OnceCell<Url2> = OnceCell::new();
    INSTANCE
        .get_or_init(|| spawn_responder_inner(ProxyTransport::Mem, proxy_url))
        .clone()
}

fn spawn_responder_inner(transport: ProxyTransport, proxy_url: Url2) -> Url2 {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let _handle = std::thread::spawn(move || {
        let mut client = make_client_inner(transport, proxy_url);
        let address = client.address.clone();
        println!("Responder {:?} Url: {}", transport, address);
        tx.send(address).unwrap();
        let f = gen_client(client.incoming.take().unwrap());
        client.runtime.get_mut().unwrap().block_on(f);
    });
    rx.recv().unwrap()
}

fn make_client_quic(
    proxy_url: Url2,
) -> (
    ghost_actor::GhostSender<TransportListener>,
    &'static std::sync::Mutex<Runtime>,
) {
    static INSTANCE: OnceCell<Client> = OnceCell::new();
    let c = INSTANCE.get_or_init(|| {
        let mut c = make_client_inner(ProxyTransport::Quic, proxy_url);

        let f = gen_client(c.incoming.take().unwrap());
        c.runtime.get_mut().unwrap().spawn(f);
        c
    });
    (c.outgoing.clone(), &c.runtime)
}

fn make_client_mem(
    proxy_url: Url2,
) -> (
    ghost_actor::GhostSender<TransportListener>,
    &'static std::sync::Mutex<Runtime>,
) {
    static INSTANCE: OnceCell<Client> = OnceCell::new();
    let c = INSTANCE.get_or_init(|| {
        let mut c = make_client_inner(ProxyTransport::Mem, proxy_url);

        let f = gen_client(c.incoming.take().unwrap());
        c.runtime.get_mut().unwrap().spawn(f);
        c
    });
    (c.outgoing.clone(), &c.runtime)
}

fn make_client_inner(transport: ProxyTransport, proxy_url: Url2) -> Client {
    let mut runtime = rt();
    let (outgoing, incoming, con_url) = runtime.block_on(async {
        let (client, incoming) = gen_cli_con(&transport, proxy_url).await.unwrap();
        let con_url = client.bound_url().await.unwrap();
        (client, incoming, con_url)
    });
    Client {
        outgoing,
        incoming: Some(incoming),
        address: con_url,
        runtime: std::sync::Mutex::new(runtime),
    }
}

fn spawn_proxy_quic() -> Url2 {
    static INSTANCE: OnceCell<Url2> = OnceCell::new();
    INSTANCE
        .get_or_init(|| spawn_proxy_inner(ProxyTransport::Quic))
        .clone()
}

fn spawn_proxy_mem() -> Url2 {
    static INSTANCE: OnceCell<Url2> = OnceCell::new();
    INSTANCE
        .get_or_init(|| spawn_proxy_inner(ProxyTransport::Mem))
        .clone()
}

fn spawn_proxy_inner(transport: ProxyTransport) -> Url2 {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let _handle = std::thread::spawn(move || {
        let mut runtime = rt();
        let handle = runtime.spawn(async move {
            let (listener, mut events) = gen_proxy_con(&transport).await.unwrap();

            let proxy_url = listener.bound_url().await.unwrap();
            println!("Proxy Url: {}", proxy_url);
            tx.send(proxy_url).unwrap();

            while let Some(evt) = events.next().await {
                match evt {
                    TransportEvent::IncomingChannel(_url, mut write, read) => {
                        let data = read.read_to_end().await;
                        write.write_and_close(data).await.unwrap();
                    }
                }
            }
            error!("proxy CLOSED!");
        });
        let r = runtime.block_on(handle);
        error!("proxy CLOSED! {:?}", r);
    });
    rx.recv().unwrap()
}

fn rt() -> Runtime {
    Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap()
}

struct Client {
    outgoing: ghost_actor::GhostSender<TransportListener>,
    incoming: Option<futures::channel::mpsc::Receiver<TransportEvent>>,
    address: Url2,
    runtime: std::sync::Mutex<Runtime>,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum ProxyTransport {
    /// Use the local in-process memory transport (faster, uses more memory)
    Mem,

    /// Use the real QUIC/UDP transport (slower, more realistic)
    Quic,
}

#[allow(dead_code)]
pub struct Opt {
    /// Transport to test with. ('mem'/'m' or 'quic'/'q')
    transport: ProxyTransport,

    /// How many client nodes should be spawned
    node_count: u32,

    /// Interval between requests per node
    request_interval_ms: u32,
}

async fn gen_base_con(
    t: &ProxyTransport,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    futures::channel::mpsc::Receiver<TransportEvent>,
)> {
    match t {
        ProxyTransport::Mem => spawn_bind_transport_mem().await,
        ProxyTransport::Quic => spawn_transport_listener_quic(Default::default()).await,
    }
}

async fn gen_proxy_con(
    t: &ProxyTransport,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    futures::channel::mpsc::Receiver<TransportEvent>,
)> {
    let (listener, events) = gen_base_con(t).await?;
    let proxy_config = ProxyConfig::local_proxy_server(
        TlsConfig::new_ephemeral().await?,
        AcceptProxyCallback::accept_all(),
    );
    spawn_kitsune_proxy_listener(proxy_config, listener, events).await
}

async fn gen_cli_con(
    t: &ProxyTransport,
    proxy_url: Url2,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    futures::channel::mpsc::Receiver<TransportEvent>,
)> {
    let (listener, events) = gen_base_con(t).await?;
    let proxy_config =
        ProxyConfig::remote_proxy_client(TlsConfig::new_ephemeral().await?, proxy_url.into());
    spawn_kitsune_proxy_listener(proxy_config, listener, events).await
}

async fn gen_client(incoming: futures::channel::mpsc::Receiver<TransportEvent>) {
    incoming
        .for_each_concurrent(NUM_RECV_CONCURRENT, move |evt| async move {
            match evt {
                TransportEvent::IncomingChannel(_url, mut write, read) => {
                    // tokio::time::delay_for(std::time::Duration::from_millis(
                    //     PROCESS_DELAY_MS.load(std::sync::atomic::Ordering::Relaxed),
                    // ))
                    // .await;
                    let data = read.read_to_end().await;
                    write.write_and_close(data).await.unwrap();
                }
            }
        })
        .await;
}
