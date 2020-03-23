//! defines the websocket listener struct

use crate::*;

/// Websocket listening / server socket.
pub struct WebsocketListener {
    local_addr: Url2,
    socket: tokio::net::TcpListener,
}

impl WebsocketListener {
    /// Get the url of the bound local listening socket.
    pub fn local_addr(&self) -> &Url2 {
        &self.local_addr
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
            std::task::Poll::Ready(Some(socket_result)) => std::task::Poll::Ready(Some(
                async move {
                    match socket_result {
                        Ok(socket) => {
                            let socket = tokio_tungstenite::accept_async(socket)
                                .await
                                .map_err(|e| Error::new(ErrorKind::Other, e))?;
                            WebsocketReceiver::priv_new(socket)
                        }
                        Err(e) => Err(Error::new(ErrorKind::Other, e)),
                    }
                }
                .boxed(),
            )),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// Bind a new websocket listening socket,
/// and begin awaiting incoming connections.
pub async fn websocket_bind(addr: Url2) -> Result<WebsocketListener> {
    let addr = url_to_addr(&addr).await?;
    let socket = match &addr {
        SocketAddr::V4(_) => net2::TcpBuilder::new_v4()?,
        SocketAddr::V6(_) => net2::TcpBuilder::new_v6()?,
    }
    .reuse_address(true)?
    .bind(addr)?
    .listen(255)?; // TODO - config?
    socket.set_nonblocking(true)?;
    let socket = tokio::net::TcpListener::from_std(socket)?;
    let local_addr = addr_to_url(socket.local_addr()?);
    Ok(WebsocketListener { local_addr, socket })
}
