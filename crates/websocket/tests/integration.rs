use holochain_websocket::*;

use std::convert::TryInto;
use tokio::stream::StreamExt;
use url2::prelude::*;

#[derive(serde::Serialize, serde::Deserialize)]
struct TestMessage(pub String);
try_from_serialized_bytes!(TestMessage);

#[tokio::test]
async fn integration_test() {
    let orig_handler = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        eprintln!("THREAD PANIC {:#?}", panic_info);
        // invoke the default handler and exit the process
        orig_handler(panic_info);
        std::process::exit(1);
    }));

    sx_types::observability::test_run().unwrap();

    let server = websocket_bind(
        url2!("ws://127.0.0.1:0"),
        std::sync::Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();
    let binding = server.local_addr().clone();

    tracing::info!(
        test = "got bound addr",
        %binding,
    );

    spawn_listener_loop(server);

    let (mut send, mut recv) =
        websocket_connect(binding, std::sync::Arc::new(WebsocketConfig::default()))
            .await
            .unwrap();

    tracing::info!(
        test = "connection success",
        remote_addr = %recv.remote_addr(),
    );

    let msg = TestMessage("test-signal".to_string());
    send.signal(msg).await.unwrap();

    let msg = TestMessage("test-request".to_string());
    let rsp: TestMessage = send.request(msg).await.unwrap();

    tracing::info!(
        test = "got response",
        data = %rsp.0,
    );

    assert_eq!("echo: test-request", &rsp.0,);

    send.close(1000, "test".to_string()).await.unwrap();

    assert_eq!(
        "WebsocketMessage::Close { close: WebsocketClosed { code: 0, reason: \"Internal Error: Protocol(\\\"Connection reset without closing handshake\\\")\" } }",
        &format!("{:?}", recv.next().await.unwrap()),
    );

    assert_eq!("None", &format!("{:?}", recv.next().await),);
}

#[tokio::test]
async fn channels_properly_close() {
    sx_types::observability::test_run().unwrap();

    let mut server = websocket_bind(
        url2!("ws://127.0.0.1:0"),
        std::sync::Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();
    let binding = server.local_addr().clone();

    tracing::info!(
        test = "got bound addr",
        %binding,
    );

    let server_handle = tokio::task::spawn(async move {
        let conn = server.next().await.unwrap();
        let (_send, mut recv) = conn.await.unwrap();
        if let WebsocketMessage::Close(_) = recv.next().await.unwrap() {
            // Simulate slow client close
            tokio::time::delay_for(std::time::Duration::from_secs(4)).await;
        } else {
            panic!("Got wrong message");
        }
    });

    let (mut send, mut recv) =
        websocket_connect(binding, std::sync::Arc::new(WebsocketConfig::default()))
            .await
            .unwrap();

    tracing::info!(
        test = "connection success",
        remote_addr = %recv.remote_addr(),
    );

    let recv_handle = tokio::task::spawn(async move {
        while let Some(msg) = recv.next().await {
            tracing::info!(?msg, "Receiver");
        }
    });

    send.close(1000, "test".to_string()).await.unwrap();

    std::mem::drop(send);
    dbg!();
    tokio::time::delay_for(std::time::Duration::from_millis(10)).await;

    tokio::time::timeout(std::time::Duration::from_secs(2), recv_handle)
        .await
        .expect("Receiver didn't close after close sent from receiver")
        .unwrap();
    server_handle.await.unwrap();
}

fn spawn_listener_loop(mut server: WebsocketListener) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        while let Some(maybe_con) = server.next().await {
            tokio::task::spawn(async move {
                let (_send, mut recv) = maybe_con.await.unwrap();
                tracing::info!(
                    test = "incoming connection",
                    remote_addr = %recv.remote_addr(),
                );
                while let Some(msg) = recv.next().await {
                    match msg {
                        WebsocketMessage::Close(close) => {
                            tracing::error!(error = ?close);
                            break;
                        }
                        WebsocketMessage::Signal(data) => {
                            let msg: TestMessage = data.try_into().unwrap();
                            tracing::info!(
                                test = "incoming signal",
                                data = %msg.0,
                            );

                            assert_eq!("test-signal", msg.0,);
                        }
                        WebsocketMessage::Request(data, respond) => {
                            let msg: TestMessage = data.try_into().unwrap();
                            tracing::info!(
                                test = "incoming message",
                                data = %msg.0,
                            );
                            let msg = TestMessage(format!("echo: {}", msg.0));
                            respond(msg.try_into().unwrap()).await.unwrap();
                        }
                    }
                }
                tracing::info!(test = "exit srv con loop");
            });
        }
        tracing::info!(test = "exit srv listen loop");
    })
}
