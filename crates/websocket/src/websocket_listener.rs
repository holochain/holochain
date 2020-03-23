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

    /// Grab the next incoming websocket.
    pub async fn accept(&mut self) -> Result<(WebsocketSender, WebsocketReceiver)> {
        let (socket, _) = self.socket.accept().await?;
        let socket = tokio_tungstenite::accept_async(socket)
            .await
            .map_err(|e| Error::new(ErrorKind::Other, e))?;
        WebsocketReceiver::priv_new(socket)
    }
}

/// Bind a new websocket listening socket,
/// and begin awaiting incoming connections.
pub async fn websocket_bind<A: ToSocketAddrs>(addr: A) -> Result<WebsocketListener> {
    let socket = tokio::net::TcpListener::bind(addr).await?;
    let local_addr = addr_to_url(socket.local_addr()?);
    Ok(WebsocketListener { local_addr, socket })
}
