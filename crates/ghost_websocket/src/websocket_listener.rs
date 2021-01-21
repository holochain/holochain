use futures::TryStreamExt;
use std::io::Error;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use stream_cancel::Trigger;
use stream_cancel::Valved;
use tracing::instrument;

use std::io::Result;
use url2::Url2;

use crate::util::addr_to_url;
use crate::util::url_to_addr;
use crate::websocket::Websocket;
use crate::WebsocketConfig;
use crate::WebsocketReceiver;
use crate::WebsocketSender;

// WebsocketListener // Stream of incoming (Sender, Receiver)
// Tokio task listening to incoming connections
// This task creates WebsocketSender Actor and
// Websocket Actor which is responsible for message id hash map
// which tracks requests to responses.
// WebsocketSender // Actor
// request()
// signal ()
// WebsocketReceiver // Stream (RequestData, Response)
// RequestData // Bytes the actual message
// Response // fn once(Message)

pub struct WebsocketListener {
    shutdown: Trigger,
    config: Arc<WebsocketConfig>,
    local_addr: Url2,
}

impl WebsocketListener {
    #[instrument(skip(config, addr))]
    pub async fn bind(
        addr: Url2,
        config: Arc<WebsocketConfig>,
    ) -> Result<(
        Self,
        impl futures::stream::Stream<Item = Result<(WebsocketSender, WebsocketReceiver)>>,
    )> {
        websocket_bind(addr, config).await
    }
    /// Shutdown the listener stream.
    pub fn close(self) {
        self.shutdown.cancel()
    }
    /// Get the url of the bound local listening socket.
    pub fn local_addr(&self) -> &Url2 {
        &self.local_addr
    }
    /// Get the config associated with this listener.
    pub fn get_config(&self) -> Arc<WebsocketConfig> {
        self.config.clone()
    }
}

async fn websocket_bind(
    addr: Url2,
    config: Arc<WebsocketConfig>,
) -> Result<(
    WebsocketListener,
    impl futures::stream::Stream<Item = Result<(WebsocketSender, WebsocketReceiver)>>,
)> {
    let addr = url_to_addr(&addr, config.scheme).await?;
    let socket = match &addr {
        SocketAddr::V4(_) => net2::TcpBuilder::new_v4()?,
        SocketAddr::V6(_) => net2::TcpBuilder::new_v6()?,
    }
    .reuse_address(true)?
    .bind(addr)?
    .listen(config.max_pending_connections as i32)?;
    socket.set_nonblocking(true)?;
    let socket = tokio::net::TcpListener::from_std(socket)?;

    let local_addr = addr_to_url(socket.local_addr()?, config.scheme);
    let socket = socket
        .map_ok({
            let config = config.clone();
            move |socket_result| connect(config.clone(), socket_result)
        })
        .try_buffer_unordered(config.max_pending_connections);
    tracing::debug!(sever_listening_on = ?local_addr);
    // .boxed();
    let (shutdown, stream) = Valved::new(socket);
    let listener = WebsocketListener {
        shutdown,
        config,
        local_addr,
    };
    Ok((listener, stream))
}

#[instrument(skip(config, socket))]
// This task creates WebsocketSender Actor and
// Websocket Actor which is responsible for message id hash map
// which tracks requests to responses.
async fn connect(
    config: Arc<WebsocketConfig>,
    // socket_result: std::io::Result<tokio::net::TcpStream>,
    socket: tokio::net::TcpStream,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    socket.set_keepalive(Some(std::time::Duration::from_secs(
        config.tcp_keepalive_s as u64,
    )))?;
    tracing::debug!(
        message = "accepted incoming raw socket",
        remote_addr = %socket.peer_addr()?,
    );
    let socket = tokio_tungstenite::accept_async_with_config(
        socket,
        Some(tungstenite::protocol::WebSocketConfig {
            max_send_queue: Some(config.max_send_queue),
            max_message_size: Some(config.max_message_size),
            max_frame_size: Some(config.max_frame_size),
        }),
    )
    .await
    .map_err(|e| Error::new(ErrorKind::Other, e))?;

    // build_websocket_pair(config, socket)
    Ok(Websocket::create_ends(config, socket))
}
