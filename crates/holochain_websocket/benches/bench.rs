use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;

use holochain_serialized_bytes::prelude::*;
use holochain_websocket::*;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;

criterion_group!(benches, simple_bench);

criterion_main!(benches);

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, Clone)]
struct TestMessage(pub String);

fn simple_bench(bench: &mut Criterion) {
    let _g = holochain_trace::test_run();

    let runtime = rt();

    let (listener_address, jh) = runtime.block_on(setup());
    let (mut send, mut recv, cjh) = runtime.block_on(setup_client(listener_address));

    let mut group = bench.benchmark_group("simple_bench");
    // group.sample_size(100);
    group.bench_function(BenchmarkId::new("client", "request"), |b| {
        b.iter(|| {
            runtime.block_on(client_request(&mut send));
        });
    });
    group.bench_function(BenchmarkId::new("client", "signal"), |b| {
        b.iter(|| {
            runtime.block_on(client_signal(&mut send));
        });
    });
    group.bench_function(BenchmarkId::new("client", "response"), |b| {
        b.iter(|| {
            runtime.block_on(client_response(&mut recv));
        });
    });
    drop(send);
    drop(recv);
    cjh.abort();
    jh.abort();
}

async fn client_request(send: &mut WebsocketSender) {
    let msg = TestMessage("test".to_string());
    // Make a request and get the echoed response
    let rsp: TestMessage = send.request(msg).await.unwrap();

    assert_eq!("echo: test", &rsp.0);
}

async fn client_signal(send: &mut WebsocketSender) {
    let msg = TestMessage("test".to_string());
    // Make a signal
    send.signal(msg).await.unwrap();
}

async fn client_response(recv: &mut tokio::sync::mpsc::Receiver<ReceiveMessage<TestMessage>>) {
    if let ReceiveMessage::Request(msg, resp) = recv.recv().await.unwrap() {
        let msg = TestMessage(format!("client: {}", msg.0));
        resp.respond(msg).await.unwrap();
    }
}

async fn setup() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    // Create a new server listening for connections
    let listener = WebsocketListener::bind(
        std::sync::Arc::new(WebsocketConfig::LISTENER_DEFAULT),
        "localhost:0",
    )
    .await
    .unwrap();

    // Get the address of the server
    let addr = listener.local_addr().unwrap();

    let jh = tokio::task::spawn(async move {
        let mut jhs = Vec::new();
        // Handle new connections
        while let Ok((send, mut recv)) = listener.accept().await {
            let jh = tokio::task::spawn(async move {
                // Receive a message and echo it back
                while let Ok(msg) = recv.recv::<TestMessage>().await {
                    // If this message is a request then we can respond
                    if let ReceiveMessage::Request(msg, resp) = msg {
                        let msg = TestMessage(format!("echo: {}", msg.0));
                        resp.respond(msg).await.unwrap();
                    }
                }
                tracing::info!("Server recv closed");
            });
            jhs.push(jh);
            let jh = tokio::task::spawn(async move {
                let msg = TestMessage("test".to_string());
                // Make a request and get the echoed response
                while let Ok(rsp) = send.request::<_, TestMessage>(msg.clone()).await {
                    assert_eq!("client: test", &rsp.0);
                }

                tracing::info!("Server send closed");
            });
            jhs.push(jh);
        }
        for jh in jhs {
            jh.await.unwrap();
        }
        tracing::info!("Server closed");
    });

    (addr, jh)
}

async fn setup_client(
    addr: std::net::SocketAddr,
) -> (
    WebsocketSender,
    tokio::sync::mpsc::Receiver<ReceiveMessage<TestMessage>>,
    tokio::task::JoinHandle<()>,
) {
    let (r_send, r_recv) = tokio::sync::mpsc::channel(32);

    // Connect the client to the server
    let (send, mut recv) = connect(std::sync::Arc::new(WebsocketConfig::CLIENT_DEFAULT), addr)
        .await
        .unwrap();

    let jh = tokio::task::spawn(async move {
        while let Ok(r) = recv.recv().await {
            r_send.send(r).await.unwrap();
        }
    });

    (send, r_recv, jh)
}

pub fn rt() -> Runtime {
    Builder::new_multi_thread().enable_all().build().unwrap()
}
