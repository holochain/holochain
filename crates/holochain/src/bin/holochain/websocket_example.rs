use crate::Port;
use holochain_websocket::*;
use std::sync::Arc;
use tokio::stream::StreamExt;
use tracing::*;
use url2::prelude::*;

pub async fn websocket_example(port: Port) -> Result<(), Box<dyn std::error::Error>> {
    let mut listener = websocket_bind(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?;
    info!("LISTENING AT: {}", listener.local_addr());
    let mut listener_handles = Vec::new();
    while let Some(maybe_con) = listener.next().await {
        listener_handles.push(tokio::task::spawn(async move {
            let (mut send_socket, mut recv_socket) = maybe_con.await.unwrap();

            info!("CONNECTION: {}", recv_socket.remote_addr());

            recv_socket.next().await;
        }));
    }
    for h in listener_handles {
        h.await?;
    }
    Ok(())
}
