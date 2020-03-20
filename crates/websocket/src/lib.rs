//! Holochain utilities for websocket serving and connecting

#![deny(missing_docs)]

pub use futures::{sink::Sink, stream::Stream};
pub use std::{
    io::{Error, ErrorKind, Result},
    net::SocketAddr,
};
pub use tokio::net::ToSocketAddrs;
pub use url2::prelude::*;

type RawSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

const SCHEME: &'static str = "ws";

/// internal helper to convert addrs to urls
fn addr_to_url(a: SocketAddr) -> Url2 {
    url2!("{}://{}", SCHEME, a)
}

/// internal helper convert urls to socket addrs for binding / connection
async fn url_to_addr(url: &Url2) -> Result<SocketAddr> {
    if url.scheme() != SCHEME || url.host_str().is_none() || url.port().is_none() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("got: '{}', expected: '{}://host:port'", SCHEME, url),
        ));
    }

    let rendered = format!("{}:{}", url.host_str().unwrap(), url.port().unwrap());

    if let Ok(mut iter) = tokio::net::lookup_host(rendered.clone()).await {
        let mut tmp = iter.next();
        let mut fallback = None;
        loop {
            if tmp.is_none() {
                break;
            }

            if tmp.as_ref().unwrap().is_ipv4() {
                return Ok(tmp.unwrap());
            }

            fallback = tmp;
            tmp = iter.next();
        }
        if let Some(addr) = fallback {
            return Ok(addr);
        }
    }

    Err(Error::new(
        ErrorKind::InvalidInput,
        format!("could not parse '{}', as 'host:port'", rendered),
    ))
}

/// Send data to the remote end of this websocket.
pub type WebsocketSender = tokio::sync::mpsc::Sender<tungstenite::Message>;

/// The read half of a websocket connection.
pub struct Websocket {
    remote_addr: Url2,
    socket: RawSocket,
    pending_outgoing_item: Option<tungstenite::Message>,
    recv_outgoing: tokio::sync::mpsc::Receiver<tungstenite::Message>,
}

// unfortunately tokio_tungstenite requires mut self for both send and recv
// so we split the sending out into a channel, and implement Stream such that
// sending and receiving are both handled simultaneously.
impl tokio::stream::Stream for Websocket {
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

impl Websocket {
    /// private constructor
    ///  - plucks the remote address
    ///  - generates our sending channel
    fn priv_new(socket: RawSocket) -> Result<(WebsocketSender, Self)> {
        let remote_addr = addr_to_url(socket.get_ref().peer_addr()?);
        let (send_outgoing, recv_outgoing) = tokio::sync::mpsc::channel(10);
        Ok((
            send_outgoing,
            Self {
                remote_addr,
                socket,
                pending_outgoing_item: None,
                recv_outgoing,
            },
        ))
    }

    /// Establish a new outgoing websocket connection.
    pub async fn connect(url: Url2) -> Result<(WebsocketSender, Self)> {
        let addr = url_to_addr(&url).await?;
        let socket = tokio::net::TcpStream::connect(addr).await?;
        let (socket, _) = tokio_tungstenite::client_async(url.as_str(), socket)
            .await
            .map_err(|e| Error::new(ErrorKind::Other, e))?;
        Websocket::priv_new(socket)
    }

    /// Get the url of the remote end of this websocket.
    pub fn remote_addr(&self) -> &Url2 {
        &self.remote_addr
    }
}

/// Websocket listening / server socket.
pub struct WebsocketListener {
    local_addr: Url2,
    socket: tokio::net::TcpListener,
}

impl WebsocketListener {
    /// Bind a new websocket listening socket,
    /// and begin awaiting incoming connections.
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let socket = tokio::net::TcpListener::bind(addr).await?;
        let local_addr = addr_to_url(socket.local_addr()?);
        Ok(Self { local_addr, socket })
    }

    /// Get the url of the bound local listening socket.
    pub fn local_addr(&self) -> &Url2 {
        &self.local_addr
    }

    /// Grab the next incoming websocket.
    pub async fn accept(&mut self) -> Result<(WebsocketSender, Websocket)> {
        let (socket, _) = self.socket.accept().await?;
        let socket = tokio_tungstenite::accept_async(socket)
            .await
            .map_err(|e| Error::new(ErrorKind::Other, e))?;
        Websocket::priv_new(socket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sanity_test() {
        use tokio::stream::StreamExt;

        let mut server = WebsocketListener::bind("127.0.0.1:0").await.unwrap();
        let binding = server.local_addr().clone();
        println!("got bound addr: {}", binding);

        tokio::task::spawn(async move {
            while let Ok((mut send, mut recv)) = server.accept().await {
                println!("got incoming connection: {}", recv.remote_addr());

                tokio::task::spawn(async move {
                    while let Some(Ok(msg)) = recv.next().await {
                        let msg = msg.into_text().unwrap();
                        println!("got incoming message: {}", msg);
                        let msg = tungstenite::Message::Text(format!("echo: {}", msg));
                        send.send(msg).await.unwrap();
                    }
                });
            }
        });

        let (mut send, mut recv) = Websocket::connect(binding).await.unwrap();
        println!("got remote addr: {}", recv.remote_addr());

        send.send(tungstenite::Message::Text("test".to_string()))
            .await
            .unwrap();

        let response = recv.next().await;
        println!("got response: {:?}", response);
    }
}
