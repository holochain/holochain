//! defines the websocket listener struct

use crate::*;
use futures::stream::BoxStream;
use futures::stream::StreamExt;

/// Websocket listening / server socket. This struct is an async Stream -
/// calling `.next().await` will give you a Future that will in turn resolve
/// to a split websocket pair (
/// [WebsocketSender](struct.WebsocketSender.html),
/// [WebsocketReceiver](struct.WebsocketReceiver.html)
/// ).
pub struct WebsocketListener {
    config: Arc<WebsocketConfig>,
    local_addr: Url2,
    socket: BoxStream<'static, Result<(WebsocketSender, WebsocketReceiver)>>,
}

impl WebsocketListener {
    /// Get the url of the bound local listening socket.
    pub fn local_addr(&self) -> &Url2 {
        &self.local_addr
    }

    /// Get the config associated with this listener.
    pub fn get_config(&self) -> Arc<WebsocketConfig> {
        self.config.clone()
    }
}

impl tokio::stream::Stream for WebsocketListener {
    type Item = Result<(WebsocketSender, WebsocketReceiver)>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<Option<Self::Item>> {
        tracing::trace!("polling");
        let p = std::pin::Pin::new(&mut self.socket);
        tokio::stream::Stream::poll_next(p, cx)
    }
}

/// Bind a new websocket listening socket, and begin awaiting incoming connections.
/// Returns a [WebsocketListener](struct.WebsocketListener.html) instance.
pub async fn websocket_bind(addr: Url2, config: Arc<WebsocketConfig>) -> Result<WebsocketListener> {
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
        .map({
            let config = config.clone();
            move |socket_result| connect(config.clone(), socket_result)
        })
        .buffer_unordered(config.max_pending_connections)
        .boxed();

    tracing::info!(
        message = "bind",
        local_addr = %local_addr,
    );
    Ok(WebsocketListener {
        config,
        local_addr,
        socket,
    })
}

/// Connects the new listener
async fn connect(
    config: Arc<WebsocketConfig>,
    socket_result: std::io::Result<tokio::net::TcpStream>,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    match socket_result {
        Ok(socket) => {
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
            build_websocket_pair(config, socket)
        }
        Err(e) => Err(Error::new(ErrorKind::Other, e)),
    }
}
