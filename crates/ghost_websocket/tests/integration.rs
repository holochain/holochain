use std::sync::Arc;

use futures::StreamExt;
use ghost_websocket::websocket_connect;
use ghost_websocket::ListenerHandle;
use ghost_websocket::ListenerItem;
use ghost_websocket::WebsocketConfig;
use ghost_websocket::WebsocketListener;
use holochain_serialized_bytes::prelude::*;
use tracing::Instrument;
use url2::url2;

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct TestString(pub String);

async fn server() -> (
    ListenerHandle,
    impl futures::stream::Stream<Item = ListenerItem>,
) {
    WebsocketListener::bind_with_handle(
        url2!("ws://127.0.0.1:0"),
        Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap()
}

#[tokio::test(threaded_scheduler)]
async fn can_connect() {
    observability::test_run().ok();
    let (handle, mut listener) = server().await;
    tokio::task::spawn(async move {
        let _ = listener
            .next()
            .instrument(tracing::debug_span!("next_server_connection"))
            .await
            .unwrap()
            .expect("Failed to connect to client");
    });
    let binding = handle.local_addr().clone();
    let _ = websocket_connect(binding, Arc::new(WebsocketConfig::default()))
        .await
        .expect("Failed to connect to server");
}

#[tokio::test(threaded_scheduler)]
async fn shutdown_listener() {
    observability::test_run().ok();
    let (handle, mut listener) = server().await;
    std::mem::drop(handle);
    assert!(listener.next().await.is_none());

    let (handle, mut listener) = server().await;
    handle.close();
    assert!(listener.next().await.is_none());

    let (handle, mut listener) = server().await;
    let jh = tokio::task::spawn(async move {
        assert!(listener.next().await.is_none());
    });
    handle.close();
    jh.await.unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn can_send_signal() {
    observability::test_run().ok();
    let (handle, mut listener) = server().await;
    let jh = tokio::task::spawn(async move {
        let (sender, receiver) = listener
            .next()
            .instrument(tracing::debug_span!("next_server_connection"))
            .await
            .unwrap()
            .unwrap();

        // - Signal client
        sender
            .signal(TestString("Hey from server".into()))
            .instrument(tracing::debug_span!("server_sending_message"))
            .await
            .unwrap();

        // - Connect to receiver
        let (_handle, mut msgs) = receiver.connect().await.unwrap();

        // - Receive signal from client
        let (msg, _) = msgs
            .next()
            .instrument(tracing::debug_span!("next_sever_recv"))
            .await
            .unwrap();
        let msg: TestString = msg.try_into().unwrap();

        assert_eq!(msg.0, "Hey from client");
    });

    // - Connect client
    let binding = handle.local_addr().clone();
    let (sender, receiver) = websocket_connect(binding, Arc::new(WebsocketConfig::default()))
        .instrument(tracing::debug_span!("client"))
        .await
        .unwrap();

    // - Connect to receiver
    let (_handle, mut msgs) = receiver.connect().await.unwrap();

    // - Receive signal from server
    let (msg, _) = msgs
        .next()
        .instrument(tracing::debug_span!("next_client_recv"))
        .await
        .unwrap();

    let msg: TestString = msg.try_into().unwrap();

    assert_eq!(msg.0, "Hey from server");

    // - Send signal to server
    sender
        .signal(TestString("Hey from client".into()))
        .instrument(tracing::debug_span!("client_sending_message"))
        .await
        .unwrap();

    jh.await.unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn can_send_request() {
    observability::test_run().ok();
    let (handle, mut listener) = server().await;
    let jh = tokio::task::spawn(async move {
        let (sender, receiver) = listener
            .next()
            .instrument(tracing::debug_span!("next_server_connection"))
            .await
            .unwrap()
            .unwrap();

        // - Request client
        let resp: TestString = sender
            .request(TestString("Hey from server".into()))
            .instrument(tracing::debug_span!("server_sending_message"))
            .await
            .unwrap();
        assert_eq!(resp.0, "Bye from client");

        // - Connect to receiver
        let (_handle, mut msgs) = receiver.connect().await.unwrap();

        // - Receive request from client
        let (msg, resp) = msgs
            .next()
            .instrument(tracing::debug_span!("next_sever_recv"))
            .await
            .unwrap();

        let msg: TestString = msg.try_into().unwrap();

        assert_eq!(msg.0, "Hey from client");

        resp(TestString("Bye from server".into()).try_into().unwrap())
            .await
            .unwrap();
    });

    // - Connect client
    let binding = handle.local_addr().clone();
    let (sender, receiver) = websocket_connect(binding, Arc::new(WebsocketConfig::default()))
        .instrument(tracing::debug_span!("client"))
        .await
        .unwrap();

    // - Connect to receiver
    let (_handle, mut msgs) = receiver.connect().await.unwrap();

    // - Receive Request from server
    let (msg, resp) = msgs
        .next()
        .instrument(tracing::debug_span!("next_client_recv"))
        .await
        .unwrap();

    let msg: TestString = msg.try_into().unwrap();

    assert_eq!(msg.0, "Hey from server");
    resp(TestString("Bye from client".into()).try_into().unwrap())
        .await
        .unwrap();

    // - Send signal to server
    let msg: TestString = sender
        .request(TestString("Hey from client".into()))
        .instrument(tracing::debug_span!("client_sending_message"))
        .await
        .unwrap();
    assert_eq!(msg.0, "Bye from server");

    jh.await.unwrap();
}
