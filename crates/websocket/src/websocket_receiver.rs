//! defines the read/recv half of a websocket pair

use crate::*;

/// The read half of a websocket connection.
pub struct WebsocketReceiver {
    remote_addr: Url2,
    socket: RawSocket,
    pending_outgoing_item: Option<tungstenite::Message>,
    recv_outgoing: tokio::sync::mpsc::Receiver<tungstenite::Message>,
}

// unfortunately tokio_tungstenite requires mut self for both send and recv
// so we split the sending out into a channel, and implement Stream such that
// sending and receiving are both handled simultaneously.
impl tokio::stream::Stream for WebsocketReceiver {
    type Item = Result<tungstenite::Message>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<Option<Self::Item>> {
        // first, if we don't have a pending outgoing item,
        // see if there is one waiting on our channel.
        if self.pending_outgoing_item.is_none() {
            let to_send = {
                let p = std::pin::Pin::new(&mut self.recv_outgoing);
                match Stream::poll_next(p, cx) {
                    std::task::Poll::Ready(s) => s,
                    std::task::Poll::Pending => None,
                }
            };
            self.pending_outgoing_item = to_send;
        }

        // now, if we have a pending outgoing item, see if our outgoing
        // stream is ready to start sending it.
        if self.pending_outgoing_item.is_some() {
            let p = std::pin::Pin::new(&mut self.socket);
            match Sink::poll_ready(p, cx) {
                std::task::Poll::Ready(Ok(_)) => {
                    let item = self.pending_outgoing_item.take().unwrap();
                    let p = std::pin::Pin::new(&mut self.socket);
                    if let Err(e) = Sink::start_send(p, item) {
                        return std::task::Poll::Ready(Some(Err(Error::new(ErrorKind::Other, e))));
                    }
                }
                std::task::Poll::Ready(Err(e)) => {
                    return std::task::Poll::Ready(Some(Err(Error::new(ErrorKind::Other, e))));
                }
                std::task::Poll::Pending => (),
            }
        }

        // now check if we have any incoming messages
        let p = std::pin::Pin::new(&mut self.socket);
        match Stream::poll_next(p, cx) {
            std::task::Poll::Ready(i) => {
                std::task::Poll::Ready(i.map(|r| r.map_err(|e| Error::new(ErrorKind::Other, e))))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl WebsocketReceiver {
    /// private constructor
    ///  - plucks the remote address
    ///  - generates our sending channel
    pub(crate) fn priv_new(socket: RawSocket) -> Result<(WebsocketSender, Self)> {
        let remote_addr = addr_to_url(socket.get_ref().peer_addr()?);
        let (send_outgoing, recv_outgoing) = tokio::sync::mpsc::channel(10);
        Ok((
            WebsocketSender::priv_new(send_outgoing),
            Self {
                remote_addr,
                socket,
                pending_outgoing_item: None,
                recv_outgoing,
            },
        ))
    }

    /// Get the url of the remote end of this websocket.
    pub fn remote_addr(&self) -> &Url2 {
        &self.remote_addr
    }
}

/// Establish a new outgoing websocket connection.
pub async fn websocket_connect(url: Url2) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let addr = url_to_addr(&url).await?;
    let socket = tokio::net::TcpStream::connect(addr).await?;
    let (socket, _) = tokio_tungstenite::client_async(url.as_str(), socket)
        .await
        .map_err(|e| Error::new(ErrorKind::Other, e))?;
    WebsocketReceiver::priv_new(socket)
}
