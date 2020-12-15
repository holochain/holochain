use holochain_websocket::*;
use std::convert::TryInto;
use tokio::stream::StreamExt;
use url2::prelude::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct BroadcastMessage(pub String);
try_from_serialized_bytes!(BroadcastMessage);

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ResponseMessage(pub String);
try_from_serialized_bytes!(ResponseMessage);

#[tokio::main(threaded_scheduler)]
async fn main() {
    observability::test_run().unwrap();

    let mut listener = websocket_bind(
        url2!("ws://127.0.0.1:12345"),
        std::sync::Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();
    eprintln!("LISTENING AT: {}", listener.local_addr());

    let (send_b, _) = tokio::sync::broadcast::channel(10);

    while let Some(maybe_con) = listener.next().await {
        let loc_send_b = send_b.clone();
        let mut loc_recv_b = send_b.subscribe();

        let (mut send_socket, mut recv_socket) = maybe_con.unwrap();

        eprintln!("CONNECTION: {}", recv_socket.remote_addr());

        // FIXME this doesn't need to be spawned as it's spawn above already
        // and there's no loop
        tokio::task::spawn(async move {
            while let Some(msg) = recv_socket.next().await {
                match msg {
                    WebsocketMessage::Signal(msg) => {
                        let msg: BroadcastMessage = msg.try_into().unwrap();
                        eprintln!("BROADCASTING: {}", msg.0);
                        loc_send_b.send(msg).unwrap();
                    }
                    WebsocketMessage::Request(msg, response) => {
                        let msg: BroadcastMessage = msg.try_into().unwrap();
                        eprintln!("RESPONDING to: {}", msg.0);
                        let response_msg = ResponseMessage(format!("Hello, {}", msg.0));
                        response(response_msg.try_into().unwrap()).await.unwrap();
                    }
                    msg => {
                        eprintln!("ERROR: {:?}", msg);
                        break;
                    }
                }
            }
        });

        tokio::task::spawn(async move {
            while let Some(Ok(msg)) = loc_recv_b.next().await {
                send_socket.signal(msg).await.unwrap();
            }
        });
    }
}
