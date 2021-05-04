use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;

use holochain_serialized_bytes::prelude::*;
use holochain_websocket::*;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use std::convert::TryInto;
use tokio_stream::StreamExt;
use url2::prelude::*;

criterion_group!(benches, simple_bench);

criterion_main!(benches);

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, Clone)]
struct TestMessage(pub String);

fn simple_bench(bench: &mut Criterion) {
    let _g = observability::test_run().ok();

    let runtime = rt();

    let (listener, listener_address, jh) = runtime.block_on(setup());
    let (mut send, mut recv) = runtime.block_on(setup_client(listener_address));

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
    runtime.block_on(async move {
        listener.close();
        drop(send);
        drop(recv);
        jh.await.unwrap();
    });
}

async fn client_request(send: &mut WebsocketSender) -> () {
    let msg = TestMessage("test".to_string());
    // Make a request and get the echoed response
    let rsp: TestMessage = send.request(msg).await.unwrap();

    assert_eq!("echo: test", &rsp.0,);
}

async fn client_signal(send: &mut WebsocketSender) -> () {
    let msg = TestMessage("test".to_string());
    // Make a signal
    send.signal(msg).await.unwrap();
}

async fn client_response(recv: &mut WebsocketReceiver) -> () {
    let (msg, resp) = recv.next().await.unwrap();
    let msg: TestMessage = msg.try_into().unwrap();
    if resp.is_request() {
        let msg = TestMessage(format!("client: {}", msg.0));
        resp.respond(msg.try_into().unwrap()).await.unwrap();
    }
}

async fn setup() -> (ListenerHandle, Url2, JoinHandle<()>) {
    // Create a new server listening for connections
    let (handle, mut listener) = WebsocketListener::bind_with_handle(
        url2!("ws://127.0.0.1:0"),
        std::sync::Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();

    let jh = tokio::task::spawn(async move {
        let mut jhs = Vec::new();
        // Handle new connections
        while let Some(Ok((mut send, mut recv))) = listener.next().await {
            let jh = tokio::task::spawn(async move {
                // Receive a message and echo it back
                while let Some((msg, resp)) = recv.next().await {
                    // Deserialize the message
                    let msg: TestMessage = msg.try_into().unwrap();
                    // If this message is a request then we can respond
                    if resp.is_request() {
                        let msg = TestMessage(format!("echo: {}", msg.0));
                        resp.respond(msg.try_into().unwrap()).await.unwrap();
                    }
                }
                tracing::info!("Server recv closed");
            });
            jhs.push(jh);
            let jh = tokio::task::spawn(async move {
                let msg = TestMessage("test".to_string());
                // Make a request and get the echoed response
                while let Ok(rsp) = send.request::<_, TestMessage>(msg.clone()).await {
                    assert_eq!("client: test", &rsp.0,);
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

    // Get the address of the server
    let addr = handle.local_addr().clone();
    (handle, addr, jh)
}

async fn setup_client(binding: Url2) -> (WebsocketSender, WebsocketReceiver) {
    // Connect the client to the server
    connect(binding, std::sync::Arc::new(WebsocketConfig::default()))
        .await
        .unwrap()
}

pub fn rt() -> Runtime {
    Builder::new_multi_thread().enable_all().build().unwrap()
}
