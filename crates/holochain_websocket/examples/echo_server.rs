use holochain_serialized_bytes::prelude::*;
use holochain_websocket::*;
use std::convert::TryInto;
use tokio::stream::StreamExt;
use url2::prelude::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct BroadcastMessage(pub String);

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct ResponseMessage(pub String);

#[tokio::main(threaded_scheduler)]
async fn main() {
    observability::test_run().unwrap();

    let (listener_handle, mut listener_stream) = WebsocketListener::bind_with_handle(
        url2!("ws://127.0.0.1:12345"),
        std::sync::Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();
    eprintln!("LISTENING AT: {}", listener_handle.local_addr());

    let (send_b, _) = tokio::sync::broadcast::channel(10);

    tokio::task::spawn(async move {
        while let Some(Ok((mut send_socket, mut recv_socket))) = listener_stream.next().await {
            let loc_send_b = send_b.clone();
            let mut loc_recv_b = send_b.subscribe();

            eprintln!("CONNECTION: {}", recv_socket.remote_addr());

            tokio::task::spawn(async move {
                while let Some((msg, resp)) = recv_socket.next().await {
                    if resp.is_request() {
                        let msg: BroadcastMessage = msg.try_into().unwrap();
                        eprintln!("RESPONDING to: {}", msg.0);
                        let response_msg = ResponseMessage(format!("Hello, {}", msg.0));
                        resp.respond(response_msg.try_into().unwrap())
                            .await
                            .unwrap();
                    } else {
                        let msg: BroadcastMessage = msg.try_into().unwrap();
                        eprintln!("BROADCASTING: {}", msg.0);
                        loc_send_b.send(msg).unwrap();
                    }
                }
            });

            tokio::task::spawn(async move {
                while let Some(Ok(msg)) = loc_recv_b.next().await {
                    send_socket.signal(msg).await.unwrap();
                }
            });
        }
    });
    tokio::signal::ctrl_c().await.unwrap();
    listener_handle.close();
    eprintln!("\nShutting down...");
    tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
}
