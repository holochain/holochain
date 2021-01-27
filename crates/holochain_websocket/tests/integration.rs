use std::sync::Arc;

use futures::StreamExt;
use holochain_serialized_bytes::prelude::*;
use holochain_websocket::connect;
use holochain_websocket::ListenerHandle;
use holochain_websocket::ListenerItem;
use holochain_websocket::WebsocketConfig;
use holochain_websocket::WebsocketListener;
use stream_cancel::Tripwire;
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

fn server_wait(
    mut listener: impl futures::stream::Stream<Item = ListenerItem> + Unpin + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        let (mut sender, mut receiver) = listener
            .next()
            .instrument(tracing::debug_span!("next_server_connection"))
            .await
            .unwrap()
            .unwrap();

        let jh = tokio::task::spawn(async move {
            sender
                .signal(TestString("Hey from server".into()))
                .instrument(tracing::debug_span!("server_sending_message"))
                .await
                .unwrap();
            let _: Option<TestString> = sender
                .request(TestString("Hey from server".into()))
                .instrument(tracing::debug_span!("server_sending_request"))
                .await
                .ok();
        });
        while let Some(_) = receiver
            .next()
            .instrument(tracing::debug_span!("server_recv_msg"))
            .await
        {}
        jh.await.unwrap();
    })
}

fn server_recv(
    mut listener: impl futures::stream::Stream<Item = ListenerItem> + Unpin + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        let (_, mut receiver) = listener
            .next()
            .instrument(tracing::debug_span!("next_server_connection"))
            .await
            .unwrap()
            .unwrap();

        while let Some((msg, _)) = receiver
            .next()
            .instrument(tracing::debug_span!("server_recv_msg"))
            .await
        {
            let msg: TestString = msg.try_into().unwrap();
            tracing::debug!(server_recv_msg = ?msg);
        }
    })
}

fn server_signal(
    mut listener: impl futures::stream::Stream<Item = ListenerItem> + Unpin + Send + 'static,
    n: usize,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        let (mut sender, _) = listener
            .next()
            .instrument(tracing::debug_span!("next_server_connection"))
            .await
            .unwrap()
            .unwrap();

        for _ in 0..n {
            sender
                .signal(TestString("Hey from server".into()))
                .instrument(tracing::debug_span!("server_sending_message"))
                .await
                .unwrap();
        }
    })
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
    let _ = connect(binding, Arc::new(WebsocketConfig::default()))
        .await
        .expect("Failed to connect to server");
}

#[tokio::test(threaded_scheduler)]
async fn can_send_signal() {
    observability::test_run().ok();
    let (handle, mut listener) = server().await;
    let jh = tokio::task::spawn(async move {
        let (mut sender, mut receiver) = listener
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

        // - Receive signal from client
        let (msg, _) = receiver
            .next()
            .instrument(tracing::debug_span!("next_sever_recv"))
            .await
            .unwrap();
        let msg: TestString = msg.try_into().unwrap();

        assert_eq!(msg.0, "Hey from client");
    });

    // - Connect client
    let binding = handle.local_addr().clone();
    let (mut sender, mut receiver) = connect(binding, Arc::new(WebsocketConfig::default()))
        .instrument(tracing::debug_span!("client"))
        .await
        .unwrap();

    // - Receive signal from server
    let (msg, _) = receiver
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
        let (mut sender, mut receiver) = listener
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

        // - Receive request from client
        let (msg, resp) = receiver
            .next()
            .instrument(tracing::debug_span!("next_server_recv"))
            .await
            .unwrap();

        let msg: TestString = msg.try_into().unwrap();

        assert_eq!(msg.0, "Hey from client");

        resp.respond(TestString("Bye from server".into()).try_into().unwrap())
            .instrument(tracing::debug_span!("server_respond"))
            .await
            .unwrap();
    });

    // - Connect client
    let binding = handle.local_addr().clone();
    let (mut sender, mut receiver) = connect(binding, Arc::new(WebsocketConfig::default()))
        .instrument(tracing::debug_span!("client"))
        .await
        .unwrap();

    // - Receive Request from server
    let (msg, resp) = receiver
        .next()
        .instrument(tracing::debug_span!("next_client_recv"))
        .await
        .unwrap();

    let msg: TestString = msg.try_into().unwrap();

    assert_eq!(msg.0, "Hey from server");
    resp.respond(TestString("Bye from client".into()).try_into().unwrap())
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

    // Close on with oneshot
    let (handle, mut listener) = server().await;
    let jh = tokio::task::spawn(async move {
        assert!(listener.next().await.is_none());
    });

    let (tx, rx) = tokio::sync::oneshot::channel();
    let cjh = tokio::task::spawn(handle.close_on(async move { rx.await.unwrap_or(true) }));
    tx.send(true).unwrap();
    cjh.await.unwrap();
    jh.await.unwrap();

    // Close on with TripWire
    let (handle, mut listener) = server().await;
    let jh = tokio::task::spawn(async move {
        assert!(listener.next().await.is_none());
    });

    let (kill, trip) = Tripwire::new();
    let cjh = tokio::task::spawn(handle.close_on(trip));
    kill.cancel();
    cjh.await.unwrap();
    jh.await.unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn shutdown_receiver() {
    observability::test_run().ok();
    let (handle, listener) = server().await;
    let s_jh = server_wait(listener);
    let binding = handle.local_addr().clone();
    let (_sender, mut receiver) = connect(binding, Arc::new(WebsocketConfig::default()))
        .instrument(tracing::debug_span!("client"))
        .await
        .unwrap();
    let rh = receiver.take_handle().unwrap();
    let c_jh = tokio::task::spawn(async move {
        receiver
            .next()
            .instrument(tracing::debug_span!("client_recv_message"))
            .await;
    });

    rh.close();
    c_jh.await.unwrap();
    handle.close();
    s_jh.await.unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn listener_shuts_down_server() {
    observability::test_run().ok();
    let (handle, listener) = server().await;
    let s_jh = server_wait(listener);
    let binding = handle.local_addr().clone();
    let (_sender, mut receiver) = connect(binding, Arc::new(WebsocketConfig::default()))
        .instrument(tracing::debug_span!("client"))
        .await
        .unwrap();

    let c_jh = tokio::task::spawn(async move {
        while let Some(_) = receiver
            .next()
            .instrument(tracing::debug_span!("client_recv_message"))
            .await
        {}
    });

    handle.close();
    s_jh.await.unwrap();
    c_jh.await.unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn client_shutdown() {
    observability::test_run().ok();
    let (handle, listener) = server().await;
    let s_jh = server_wait(listener);
    let binding = handle.local_addr().clone();
    let (_sender, mut receiver) = connect(binding, Arc::new(WebsocketConfig::default()))
        .instrument(tracing::debug_span!("client"))
        .await
        .unwrap();
    let rh = receiver.take_handle().unwrap();
    let c_jh = tokio::task::spawn(async move {
        while let Some(_) = receiver
            .next()
            .instrument(tracing::debug_span!("client_recv_message"))
            .await
        {}
    });

    rh.close();
    c_jh.await.unwrap();
    s_jh.await.unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn drop_sender() {
    observability::test_run().ok();
    let (handle, listener) = server().await;
    let s_jh = server_signal(listener, 10);
    let binding = handle.local_addr().clone();
    let (_, mut receiver) = connect(binding, Arc::new(WebsocketConfig::default()))
        .instrument(tracing::debug_span!("client"))
        .await
        .unwrap();
    let c_jh = tokio::task::spawn(async move {
        for _ in 0..10 {
            receiver
                .next()
                .instrument(tracing::debug_span!("server_recv_message"))
                .await
                .unwrap();
        }
    });

    c_jh.await.unwrap();
    s_jh.await.unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn drop_receiver() {
    observability::test_run().ok();
    let (handle, listener) = server().await;
    let s_jh = server_recv(listener);
    let binding = handle.local_addr().clone();
    let (mut sender, _) = connect(binding, Arc::new(WebsocketConfig::default()))
        .instrument(tracing::debug_span!("client"))
        .await
        .unwrap();
    let c_jh = tokio::task::spawn(async move {
        for _ in 0..10 {
            sender
                .signal(TestString("Hey from client".into()))
                .instrument(tracing::debug_span!("client_sending_message"))
                .await
                .unwrap();
        }
    });

    c_jh.await.unwrap();
    s_jh.await.unwrap();
}
