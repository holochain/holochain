//! defines the read/recv half of a websocket pair

use crate::*;

/// When a websocket is closed gracefully from the remote end,
/// this item is included in the ConnectionReset error message.
#[derive(Debug, Clone)]
pub struct WebsocketClosed {
    /// Websocket canonical close code.
    pub code: u16,

    /// Subjective close reason.
    pub reason: String,
}

/// Callback for responding to incoming RPC requests
pub type WebsocketRespond =
    Box<dyn FnOnce(SerializedBytes) -> BoxFuture<'static, Result<()>> + 'static + Send + Sync>;

/// You can receive Signals or Requests from the remote side of the websocket.
pub enum WebsocketMessage {
    /// A signal does not require a response.
    Signal(SerializedBytes),

    /// A request that is expecting a response.
    Request(SerializedBytes, WebsocketRespond),

    /// The websocket was closed - don't expect any more messages.
    Close(WebsocketClosed),
}

impl PartialEq for WebsocketMessage {
    fn eq(&self, other: &WebsocketMessage) -> bool {
        match (self, other) {
            (WebsocketMessage::Request(sb1, _), WebsocketMessage::Request(sb2, _)) => sb1 == sb2,
            (s, o) => s == o,
        }
    }
}

impl std::fmt::Debug for WebsocketMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebsocketMessage::Signal(data) => f
                .debug_struct("WebsocketMessage::Signal")
                .field("bytes", &data.bytes().len())
                .finish(),
            WebsocketMessage::Request(data, _) => f
                .debug_struct("WebsocketMessage::Request")
                .field("bytes", &data.bytes().len())
                .finish(),
            WebsocketMessage::Close(close) => f
                .debug_struct("WebsocketMessage::Close")
                .field("close", &close)
                .finish(),
        }
    }
}

/// internal publish messages to the WebsocketReceiver Sender.
pub(crate) type ToWebsocketReceiverSender = tokio::sync::mpsc::Sender<WebsocketMessage>;

/// internal publish messages to the WebsocketReceiver Receiver.
type ToWebsocketReceiverReceiver = tokio::sync::mpsc::Receiver<WebsocketMessage>;

/// The Receiver/Read half of a split websocket. Use this to receive signals
/// and respond to rpc requests from the remote end of this websocket connection.
/// This struct is an async Stream - calling `.next().await` will give you a
/// [WebsocketMessage](enum.WebsocketMessage.html).
pub struct WebsocketReceiver {
    remote_addr: Url2,
    recv: ToWebsocketReceiverReceiver,
}

impl WebsocketReceiver {
    /// get the remote url this websocket is connected to.
    pub fn remote_addr(&self) -> &Url2 {
        &self.remote_addr
    }

    /// internal constructor
    fn priv_new(remote_addr: Url2, recv: ToWebsocketReceiverReceiver) -> Self {
        Self { remote_addr, recv }
    }
}

impl tokio::stream::Stream for WebsocketReceiver {
    type Item = WebsocketMessage;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<Option<Self::Item>> {
        let p = std::pin::Pin::new(&mut self.recv);
        tokio::stream::Stream::poll_next(p, cx)
    }
}

/// Establish a new outgoing websocket connection. Returns a split websocket
/// connection pair: (
/// [WebsocketSender](struct.WebsocketSender.html),
/// [WebsocketReceiver](struct.WebsocketReceiver.html)
/// ).
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
    build_websocket_pair(config, socket)
}

/// internal set up the tokio tasks that keep a websocket running
/// and produce the public (WebsocketSender, WebsocketReceiver) pair.
pub(crate) fn build_websocket_pair(
    config: Arc<WebsocketConfig>,
    socket: RawSocket,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let remote_addr = url2!(
        "{}#{}",
        addr_to_url(socket.get_ref().peer_addr()?, config.scheme),
        nanoid::nanoid!(),
    );

    // split the sink and stream so we can handle them simultaneously
    use futures::stream::StreamExt;
    let (raw_sink, raw_stream) = socket.split();

    // create our public channel for the WebsocketReceiver
    let (send_pub, recv_pub) = tokio::sync::mpsc::channel(config.max_send_queue);

    // the socket sink task handles sending outgoing data
    let send_sink = task_socket_sink::build(config.clone(), remote_addr.clone(), raw_sink);

    // the dispatch task gathers:
    //  - register responses from the WebsocketSender
    //  - incoming data from the socket stream task
    let send_dispatch =
        task_dispatch_incoming::build(config, remote_addr.clone(), send_pub, send_sink.clone());

    // the socket stream task forwards incoming data to the dispatcher
    // it also responds to pings by directly sending to the sink
    task_socket_stream::build(
        remote_addr.clone(),
        send_sink.clone(),
        send_dispatch.clone(),
        raw_stream,
    );

    // return our send / recv pair
    Ok((
        WebsocketSender::priv_new(send_sink, send_dispatch),
        WebsocketReceiver::priv_new(remote_addr, recv_pub),
    ))
}
