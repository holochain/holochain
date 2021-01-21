// config for new listeners and connections
mod websocket_config;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::sync::Arc;

use tracing::instrument;
use url2::Url2;
use util::url_to_addr;
use websocket::Websocket;
pub use websocket_config::*;

// the listener / server socket
mod websocket_listener;
pub use websocket_listener::*;

mod websocket_sender;
pub use websocket_sender::*;

// the receiver/read half of the split websocket
mod websocket_receiver;
pub use websocket_receiver::*;

#[instrument(skip(config))]
pub async fn websocket_connect(
    url: Url2,
    config: Arc<WebsocketConfig>,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let addr = url_to_addr(&url, config.scheme).await?;
    let socket = tokio::net::TcpStream::connect(addr).await?;
    socket.set_keepalive(Some(std::time::Duration::from_secs(
        config.tcp_keepalive_s as u64,
    )))?;
    let (socket, _) = tokio_tungstenite::client_async_with_config(
        url.as_str(),
        socket,
        Some(config.to_tungstenite()),
    )
    .await
    .map_err(|e| Error::new(ErrorKind::Other, e))?;
    tracing::debug!("Client connected");
    Ok(Websocket::create_ends(config, socket))
}

pub struct WebsocketMessage(pub String);

mod websocket;

mod util;