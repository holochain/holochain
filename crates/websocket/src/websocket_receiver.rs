//! defines the read/recv half of a websocket pair

use crate::*;

/// Callback for responding to incoming RPC requests
pub type WebsocketRespond =
    Box<dyn FnOnce(SerializedBytes) -> BoxFuture<'static, Result<()>> + 'static + Send>;

/// You can receive Signals or Requests from the remote side of the websocket.
pub enum WebsocketMessage {
    /// A signal does not require a response.
    Signal(SerializedBytes),

    /// A request that is expecting a response.
    Request(SerializedBytes, WebsocketRespond),
}

/// When a websocket is closed gracefully from the remote end,
/// this item is included in the ConnectionReset error message.
#[derive(Debug, Clone)]
pub struct WebsocketClosed {
    /// Websocket canonical close code.
    pub code: u16,

    /// Subjective close reason.
    pub reason: String,
}

impl std::fmt::Display for WebsocketClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for WebsocketClosed {}

/// internal request item
struct RequestItem {
    expires_at: std::time::Instant,
    respond: Option<tokio::sync::oneshot::Sender<Result<Vec<u8>>>>,
    span: tracing::Span,
}

/// internal websocket receiver state items
/// this allows us to drop all this in the case of a close
struct WebsocketReceiverInner {
    socket: RawSocket,
    send_outgoing: RawSender,
    recv_outgoing: RawReceiver,
    pending_requests: std::collections::HashMap<String, RequestItem>,
    pongs: Vec<Vec<u8>>,
}

/// The read half of a websocket connection.
/// Note that due to underlying types this receiver must be awaited
/// for outgoing messages to be sent as well.
pub struct WebsocketReceiver {
    config: Arc<WebsocketConfig>,
    remote_addr: Url2,
    inner: Option<WebsocketReceiverInner>,
}

// unfortunately tokio_tungstenite requires mut self for both send and recv
// so we split the sending out into a channel, and implement Stream such that
// sending and receiving are both handled simultaneously.
impl tokio::stream::Stream for WebsocketReceiver {
    type Item = Result<WebsocketMessage>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.inner.is_none() {
            return std::task::Poll::Ready(None);
        }

        // first, check to see if we are ready to send any outgoing items
        if let Err(e) = Self::priv_poll_can_send(std::pin::Pin::new(&mut self), cx) {
            drop(self.inner.take());
            return std::task::Poll::Ready(Some(Err(e)));
        }

        // now check if we have any incoming messages
        let p = std::pin::Pin::new(&mut self.inner.as_mut().unwrap().socket);
        let result = match futures::stream::Stream::poll_next(p, cx) {
            std::task::Poll::Ready(Some(Ok(i))) => {
                match i {
                    tungstenite::Message::Close(close) => {
                        let (code, reason) = match close {
                            Some(frame) => (frame.code.into(), frame.reason.into()),
                            None => (0_u16, "".to_string()),
                        };
                        tracing::info!(
                            message = "closing websocket",
                            remote_addr = %self.remote_addr,
                            code = %code,
                            reason = %reason,
                        );
                        drop(self.inner.take());
                        return std::task::Poll::Ready(Some(Err(Error::new(
                            ErrorKind::ConnectionReset,
                            WebsocketClosed { code, reason },
                        ))));
                    }
                    tungstenite::Message::Ping(data) => {
                        self.inner.as_mut().unwrap().pongs.push(data);
                        // trigger wake - there may be more data
                        cx.waker().wake_by_ref();
                        return std::task::Poll::Pending;
                    }
                    tungstenite::Message::Pong(_) => {
                        // trigger wake - there may be more data
                        cx.waker().wake_by_ref();
                        return std::task::Poll::Pending;
                    }
                    _ => (),
                }
                match self.priv_check_incoming(i.into_data()) {
                    Ok(Some(output)) => std::task::Poll::Ready(Some(Ok(output))),
                    Ok(None) => {
                        // trigger wake - there may be more data
                        cx.waker().wake_by_ref();
                        std::task::Poll::Pending
                    }
                    Err(e) => {
                        tracing::info!(
                            message = "closing websocket",
                            remote_addr = %self.remote_addr,
                            error = ?e,
                        );
                        drop(self.inner.take());
                        return std::task::Poll::Ready(Some(Err(e)));
                    }
                }
            }
            std::task::Poll::Ready(Some(Err(e))) => {
                tracing::info!(
                    message = "closing websocket",
                    remote_addr = %self.remote_addr,
                    error = ?e,
                );
                drop(self.inner.take());
                return std::task::Poll::Ready(Some(Err(Error::new(ErrorKind::Other, e))));
            }
            std::task::Poll::Ready(None) => {
                tracing::info!(
                    message = "closing websocket",
                    remote_addr = %self.remote_addr,
                    error = "stream end",
                );
                drop(self.inner.take());
                return std::task::Poll::Ready(None);
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        };

        // finally clean up any expired pending requests
        self.priv_prune_pending();

        result
    }
}

impl WebsocketReceiver {
    /// Get the url of the remote end of this websocket.
    pub fn remote_addr(&self) -> &Url2 {
        &self.remote_addr
    }

    /// Get the config associated with this listener.
    pub fn get_config(&self) -> Arc<WebsocketConfig> {
        self.config.clone()
    }

    // -- private -- //

    /// private constructor
    ///  - plucks the remote address
    ///  - generates our sending channel
    pub(crate) fn priv_new(
        config: Arc<WebsocketConfig>,
        socket: RawSocket,
    ) -> Result<(WebsocketSender, Self)> {
        let remote_addr = addr_to_url(socket.get_ref().peer_addr()?, config.scheme);
        tracing::info!(
            message = "websocket handshake success",
            remote_addr = %remote_addr,
        );
        let (send_outgoing, recv_outgoing) = tokio::sync::mpsc::channel(10);
        Ok((
            WebsocketSender::priv_new(send_outgoing.clone()),
            Self {
                config,
                remote_addr,
                inner: Some(WebsocketReceiverInner {
                    socket,
                    send_outgoing,
                    recv_outgoing,
                    pending_requests: std::collections::HashMap::new(),
                    pongs: Vec::new(),
                }),
            },
        ))
    }

    /// internal check for sending outgoing messages
    fn priv_poll_send_item(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> Result<()> {
        let p = std::pin::Pin::new(&mut self.inner.as_mut().unwrap().recv_outgoing);
        match tokio::stream::Stream::poll_next(p, cx) {
            std::task::Poll::Ready(Some((msg, respond))) => {
                // prepare the item for send
                let msg = match self.priv_prep_send(msg, respond) {
                    Ok(msg) => msg,
                    Err(e) => {
                        return Err(e);
                    }
                };

                tracing::trace!(
                    message = "sending data",
                    byte_count = %msg.len(),
                );

                // send the item
                let p = std::pin::Pin::new(&mut self.inner.as_mut().unwrap().socket);
                if let Err(e) = futures::sink::Sink::start_send(p, msg) {
                    return Err(Error::new(ErrorKind::Other, e));
                }

                // trigger wake - there may be more data
                cx.waker().wake_by_ref();
            }
            std::task::Poll::Ready(None) => (),
            std::task::Poll::Pending => (),
        }
        Ok(())
    }

    /// internal check for sending outgoing messages
    fn priv_poll_can_send(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> Result<()> {
        let p = std::pin::Pin::new(&mut self.inner.as_mut().unwrap().socket);
        match futures::sink::Sink::poll_ready(p, cx) {
            std::task::Poll::Ready(Ok(_)) => {
                if self.inner.as_ref().unwrap().pongs.is_empty() {
                    self.priv_poll_send_item(cx)?;
                } else {
                    tracing::trace!(message = "sending pong response to ping");

                    let pong_data = self.inner.as_mut().unwrap().pongs.remove(0);
                    let p = std::pin::Pin::new(&mut self.inner.as_mut().unwrap().socket);
                    if let Err(e) =
                        futures::sink::Sink::start_send(p, tungstenite::Message::Pong(pong_data))
                    {
                        return Err(Error::new(ErrorKind::Other, e));
                    }
                    // trigger wake - there may be more data
                    cx.waker().wake_by_ref();
                }
            }
            std::task::Poll::Ready(Err(e)) => {
                return Err(Error::new(ErrorKind::Other, e));
            }
            std::task::Poll::Pending => (),
        }
        Ok(())
    }

    /// internal helper for tracking request/response ids
    fn priv_prep_send(
        &mut self,
        msg: SendMessage,
        respond: Option<tokio::sync::oneshot::Sender<Result<Vec<u8>>>>,
    ) -> Result<tungstenite::Message> {
        match msg {
            // currently we're not exposing ping functionality
            // but this is how we'd do it:
            //SendMessage::Ping => Ok(tungstenite::Message::Ping(Vec::with_capacity(0))),
            SendMessage::Close { code, reason } => Ok(tungstenite::Message::Close(Some(
                tungstenite::protocol::CloseFrame {
                    code: code.into(),
                    reason: reason.into(),
                },
            ))),
            SendMessage::Message(msg, span) => {
                let _g = span.enter();
                let span = tracing::debug_span!("prep_send");
                let _g = span.enter();
                if let Some(id) = msg.clone_id() {
                    if respond.is_some() {
                        self.inner.as_mut().unwrap().pending_requests.insert(
                            id,
                            RequestItem {
                                expires_at: std::time::Instant::now()
                                    .checked_add(std::time::Duration::from_secs(
                                        self.config.default_request_timeout_s as u64,
                                    ))
                                    .expect("can set expires_at"),
                                respond,
                                span: tracing::debug_span!("await_response"),
                            },
                        );
                    }
                }
                let bytes: SerializedBytes = msg.try_into()?;
                let bytes: Vec<u8> = UnsafeBytes::from(bytes).into();
                Ok(tungstenite::Message::Binary(bytes))
            }
        }
    }

    /// internal helper for processing incoming data
    fn priv_check_incoming(&mut self, msg: Vec<u8>) -> Result<Option<WebsocketMessage>> {
        let bytes: SerializedBytes = UnsafeBytes::from(msg).into();
        let msg: Message = bytes.try_into()?;
        match msg {
            Message::Signal { data } => {
                tracing::trace!(
                    message = "recieved signal",
                    data = %String::from_utf8_lossy(&data),
                );
                // we got a signal
                Ok(Some(WebsocketMessage::Signal(
                    UnsafeBytes::from(data).into(),
                )))
            }
            Message::Request { id, data } => {
                tracing::trace!(
                    message = "recieved request",
                    %id,
                    data = %String::from_utf8_lossy(&data),
                );
                // we got a request
                //  - set up a responder callback
                //  - notify our stream subscriber of the message
                let mut sender = self.inner.as_ref().unwrap().send_outgoing.clone();
                let respond: WebsocketRespond = Box::new(|data| {
                    let span = tracing::debug_span!("respond");
                    async move {
                        let msg = Message::Response {
                            id,
                            data: UnsafeBytes::from(data).into(),
                        };
                        sender
                            .send((SendMessage::Message(msg, span), None))
                            .await
                            .map_err(|e| Error::new(ErrorKind::Other, e))?;
                        Ok(())
                    }
                    .boxed()
                });
                Ok(Some(WebsocketMessage::Request(
                    UnsafeBytes::from(data).into(),
                    respond,
                )))
            }
            Message::Response { id, data } => {
                tracing::trace!(
                    message = "recieved response",
                    %id,
                    data = %String::from_utf8_lossy(&data),
                );

                // check our pending table / match up this response
                if let Some(mut item) = self.inner.as_mut().unwrap().pending_requests.remove(&id) {
                    let _g = item.span.enter();
                    if let Some(respond) = item.respond.take() {
                        if let Err(e) = respond.send(Ok(data)) {
                            tracing::warn!(error = ?e);
                        }
                    }
                }
                Ok(None)
            }
        }
    }

    /// prune any expired pending responses
    fn priv_prune_pending(&mut self) {
        let now = std::time::Instant::now();
        self.inner
            .as_mut()
            .unwrap()
            .pending_requests
            .retain(|_k, v| {
                if v.expires_at < now {
                    if let Some(respond) = v.respond.take() {
                        if let Err(e) = respond.send(Err(ErrorKind::TimedOut.into())) {
                            tracing::warn!(error = ?e);
                        }
                    }
                    false
                } else {
                    true
                }
            });
    }
}

/// Establish a new outgoing websocket connection.
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
    WebsocketReceiver::priv_new(config, socket)
}

/// bla
pub type WSink = futures::stream::SplitSink<RawSocket, tungstenite::Message>;

/// bla
pub type WStream = futures::stream::SplitStream<RawSocket>;

/// bla
pub async fn websocket_connect_split(
    url: Url2,
    config: Arc<WebsocketConfig>,
) -> Result<(WSink, WStream)> {
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
    use futures::stream::StreamExt;
    let (write, read): (WSink, WStream) = socket.split();
    Ok((write, read))
}
