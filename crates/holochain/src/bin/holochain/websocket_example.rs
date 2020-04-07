
use tokio::stream::StreamExt;
use tracing::*;
use url2::prelude::*;
use crate::Port;
use holochain_websocket::*;
use std::sync::Arc;

pub async fn websocket_example(port: Port) -> Result<(), Box<dyn std::error::Error>> {
    let mut listener = websocket_bind(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?;
    info!("LISTENING AT: {}", listener.local_addr());
    while let Some(maybe_con) = listener.next().await {
    }
    Ok(())
}