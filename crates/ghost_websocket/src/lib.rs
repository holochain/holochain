// config for new listeners and connections
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::sync::Arc;

use stream_cancel::Valve;
use tracing::instrument;
use url2::Url2;
use util::url_to_addr;
use websocket::Websocket;

mod websocket_config;
pub use websocket_config::*;

mod error;
pub use error::*;

// the listener / server socket
mod websocket_listener;
pub use websocket_listener::*;

mod websocket_sender;
pub use websocket_sender::*;

// the receiver/read half of the split websocket
mod websocket_receiver;
pub use websocket_receiver::*;

#[instrument(skip(config))]
pub async fn connect(
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

    // Noop valve because we don't have a listener to shutdown the
    // ends when creating a client
    let (exit, valve) = Valve::new();
    exit.disable();
    Ok(Websocket::create_ends(config, socket, valve))
}

mod websocket;

mod util;
