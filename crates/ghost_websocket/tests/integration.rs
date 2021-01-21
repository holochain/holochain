use std::sync::Arc;

use futures::StreamExt;
use ghost_websocket::websocket_connect;
use ghost_websocket::WebsocketConfig;
use ghost_websocket::WebsocketListener;
use tracing::Instrument;
use url2::url2;

#[tokio::test(threaded_scheduler)]
async fn can_connect() {
    observability::test_run().ok();
    let (handle, mut listener) = WebsocketListener::bind(
        url2!("ws://127.0.0.1:0"),
        Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();
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
    let (handle, mut listener) = WebsocketListener::bind(
        url2!("ws://127.0.0.1:0"),
        Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();
    std::mem::drop(handle);
    assert!(listener.next().await.is_none());

    let (handle, mut listener) = WebsocketListener::bind(
        url2!("ws://127.0.0.1:0"),
        Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();
    handle.close();
    assert!(listener.next().await.is_none());

    let (handle, mut listener) = WebsocketListener::bind(
        url2!("ws://127.0.0.1:0"),
        Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();
    tokio::task::spawn(async move {
        assert!(listener.next().await.is_none());
    });
    handle.close();
    tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
}

#[tokio::test(threaded_scheduler)]
async fn can_send_message() {
    observability::test_run().ok();
    let (handle, mut listener) = WebsocketListener::bind(
        url2!("ws://127.0.0.1:0"),
        Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();
    tokio::task::spawn(async move {
        let (sender, _) = listener
            .next()
            .instrument(tracing::debug_span!("next_server_connection"))
            .await
            .unwrap()
            .unwrap();
        sender
            .signal("Hey from server")
            .instrument(tracing::debug_span!("server_sending_message"))
            .await
            .unwrap();
    });
    let binding = handle.local_addr().clone();
    let (_, receiver) = websocket_connect(binding, Arc::new(WebsocketConfig::default()))
        .await
        .unwrap();
    let (_handle, mut msgs) = receiver.connect().await.unwrap();
    let (msg, _) = msgs
        .next()
        .instrument(tracing::debug_span!("next_client_recv"))
        .await
        .unwrap();
    // unwrap_to!(msg => )
    assert_eq!(msg, "Hey from server");
}
