use holochain_serialized_bytes::prelude::*;
use holochain_websocket::*;

use std::convert::TryInto;
use tokio::stream::StreamExt;
use url2::prelude::*;

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
struct TestMessage(pub String);

#[tokio::main]
async fn main() {
    // Create a new server listening for connections
    let mut server = WebsocketListener::bind(
        url2!("ws://127.0.0.1:0"),
        std::sync::Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();

    // Get the address of the server
    let binding = server.local_addr().clone();

    tokio::task::spawn(async move {
        // Handle new connections
        while let Some(Ok((_send, mut recv))) = server.next().await {
            tokio::task::spawn(async move {
                // Receive a message and echo it back
                if let Some((msg, resp)) = recv.next().await {
                    // Deserialize the message
                    let msg: TestMessage = msg.try_into().unwrap();
                    // If this message is a request then we can respond
                    if resp.is_request() {
                        let msg = TestMessage(format!("echo: {}", msg.0));
                        resp.respond(msg.try_into().unwrap()).await.unwrap();
                    }
                }
            });
        }
    });

    // Connect the client to the server
    let (mut send, _recv) = connect(binding, std::sync::Arc::new(WebsocketConfig::default()))
        .await
        .unwrap();

    let msg = TestMessage("test".to_string());
    // Make a request and get the echoed response
    let rsp: TestMessage = send.request(msg).await.unwrap();

    assert_eq!("echo: test", &rsp.0,);
}
