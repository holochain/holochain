use std::net::ToSocketAddrs;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::Criterion;

use holochain_serialized_bytes::prelude::*;
use holochain_websocket::*;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;

criterion_group!(benches, full_connect);

criterion_main!(benches);

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, Clone, PartialEq)]
struct TestMessage(String);

fn full_connect(bench: &mut Criterion) {
    let _g = holochain_trace::test_run().ok();

    let runtime = rt();

    let config = std::sync::Arc::new(WebsocketConfig::default());
    let config = &config;

    let hello = TestMessage("hello".to_string());
    let hello = &hello;
    let world = TestMessage("world".to_string());
    let world = &world;

    bench.bench_function("full_connect", |b| b.iter(|| {
        runtime.block_on(async move {
            let bind_addr = "localhost:0".to_socket_addrs().unwrap().next().unwrap();
            let l = WebsocketListener::bind(config.clone(), bind_addr).await.unwrap();

            let port = l.local_addr().unwrap().port();

            let b1 = std::sync::Arc::new(tokio::sync::Barrier::new(2));
            let b2 = b1.clone();

            tokio::join!(async {
                let (_s, mut r) = l.accept().await.unwrap();
                match r.recv::<TestMessage>().await.unwrap() {
                    ReceiveMessage::Request(msg, respond) => {
                        assert_eq!(hello, &msg);
                        respond.respond(world.clone()).await.unwrap();
                    }
                    _ => panic!(),
                }
                b1.wait().await;
            }, async {
                let (s, mut r) = connect(config.clone(), format!("localhost:{port}").to_socket_addrs().unwrap().next().unwrap()).await.unwrap();
                tokio::select! {
                    _ = r.recv::<TestMessage>() => (),
                    _ = async {
                        assert_eq!(world, &s.request::<_, TestMessage>(hello.clone()).await.unwrap());
                    } => (),
                }
                b2.wait().await;
            });
        });
    }));
}

pub fn rt() -> Runtime {
    Builder::new_multi_thread().enable_all().build().unwrap()
}
