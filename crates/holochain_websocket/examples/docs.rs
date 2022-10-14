use holochain_serialized_bytes::prelude::*;
use holochain_websocket::*;
use std::time::Duration;
use url2::url2;

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
struct TestMessage(pub String);

#[tokio::main]
async fn main() {
    let (send, _) = connect(
        url2!("ws://127.0.0.1:12345"),
        std::sync::Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();

    send.signal(TestMessage("Hey".to_string())).await.unwrap();
    let resp: TestMessage = send.request(TestMessage("Hey".to_string())).await.unwrap();
    println!("Got {:?}", resp);

    match send
        .request_timeout(TestMessage("Hey".to_string()), Duration::from_secs(1))
        .await
    {
        Ok(r) => {
            let resp: TestMessage = r;
            println!("Got {:?}", resp);
        }
        Err(WebsocketError::RespTimeout) => eprintln!("Failed to get a response in 1 second"),
        Err(e) => eprintln!("Got an error sending a request {:?}", e),
    }
}

// let mut listener = WebsocketListener::bind(
//     url2!("ws://127.0.0.1:12345"),
//     std::sync::Arc::new(WebsocketConfig::default()),
// )
// .await
// .unwrap();

// while let Some(Ok((_send, _recv))) = listener.next().await {
//     // New connection
// }
// let (tx, rx) = tokio::sync::oneshot::channel();
// tokio::task::spawn(listener_handle.close_on(async move { rx.await.unwrap_or(true) }));
// tx.send(true).unwrap();

// while let Some((msg, resp)) = recv.next().await {
//     if resp.is_request() {
//         resp.respond(msg).await.unwrap();
//     }
// }
