//! defines the websocket listener struct

use crate::*;

/// Websocket listening / server socket.
pub struct WebsocketListener {
    config: Arc<WebsocketConfig>,
    local_addr: Url2,
    socket: tokio::net::TcpListener,
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
    type Item = BoxFuture<'static, Result<(WebsocketSender, WebsocketReceiver)>>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<Option<Self::Item>> {
        let p = std::pin::Pin::new(&mut self.socket);
        match tokio::stream::Stream::poll_next(p, cx) {
            std::task::Poll::Ready(Some(socket_result)) => {
                let config = self.config.clone();
                std::task::Poll::Ready(Some(
                    async move {
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
                    .boxed(),
                ))
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// Bind a new websocket listening socket,
/// and begin awaiting incoming connections.
pub async fn websocket_bind(addr: Url2, config: Arc<WebsocketConfig>) -> Result<WebsocketListener> {
    let addr = url_to_addr(&addr, config.scheme).await?;
    let socket = match &addr {
        SocketAddr::V4(_) => net2::TcpBuilder::new_v4()?,
        SocketAddr::V6(_) => net2::TcpBuilder::new_v6()?,
    }
    .reuse_address(true)?
    .bind(addr)?
    .listen(255)?; // TODO - config?
    socket.set_nonblocking(true)?;
    let socket = tokio::net::TcpListener::from_std(socket)?;
    let local_addr = addr_to_url(socket.local_addr()?, config.scheme);
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
