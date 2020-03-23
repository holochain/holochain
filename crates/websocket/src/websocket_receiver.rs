//! defines the read/recv half of a websocket pair

use crate::*;

const REQUEST_TIMEOUT_MS: u64 = 30000; // 30 seconds

/// Callback for responding to incoming RPC requests
pub type WebsocketRespond =
    Box<dyn FnOnce(SerializedBytes) -> BoxFuture<'static, Result<()>> + 'static + Send>;

/// internal request item
struct RequestItem {
    expires_at: std::time::Instant,
    respond: Option<tokio::sync::oneshot::Sender<Result<Vec<u8>>>>,
}

/// The read half of a websocket connection.
/// Note that due to underlying types this receiver must be awaited
/// for outgoing messages to be sent as well.
pub struct WebsocketReceiver {
    remote_addr: Url2,
    socket: RawSocket,
    send_outgoing: RawSender,
    recv_outgoing: RawReceiver,
    pending_requests: std::collections::HashMap<String, RequestItem>,
}

// unfortunately tokio_tungstenite requires mut self for both send and recv
// so we split the sending out into a channel, and implement Stream such that
// sending and receiving are both handled simultaneously.
impl tokio::stream::Stream for WebsocketReceiver {
    type Item = Result<(SerializedBytes, WebsocketRespond)>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<Option<Self::Item>> {
        // first, check to see if we are ready to send any outgoing items
        let p = std::pin::Pin::new(&mut self.socket);
        match futures::sink::Sink::poll_ready(p, cx) {
            std::task::Poll::Ready(Ok(_)) => {
                // we are ready to send - check if there is anything to send
                let p = std::pin::Pin::new(&mut self.recv_outgoing);
                match futures::stream::Stream::poll_next(p, cx) {
                    std::task::Poll::Ready(Some((msg, respond))) => {
                        // prepare the item for send
                        let msg = match self.priv_prep_send(msg, respond) {
                            Ok(msg) => msg,
                            Err(e) => {
                                return std::task::Poll::Ready(Some(Err(e)));
                            }
                        };

                        // send the item
                        let p = std::pin::Pin::new(&mut self.socket);
                        if let Err(e) =
                            futures::sink::Sink::start_send(p, tungstenite::Message::Binary(msg))
                        {
                            return std::task::Poll::Ready(Some(Err(Error::new(
                                ErrorKind::Other,
                                e,
                            ))));
                        }
                    }
                    std::task::Poll::Ready(None) => (),
                    std::task::Poll::Pending => (),
                }
            }
            std::task::Poll::Ready(Err(e)) => {
                return std::task::Poll::Ready(Some(Err(Error::new(ErrorKind::Other, e))));
            }
            std::task::Poll::Pending => (),
        }

        // now check if we have any incoming messages
        let p = std::pin::Pin::new(&mut self.socket);
        let result = match futures::stream::Stream::poll_next(p, cx) {
            std::task::Poll::Ready(Some(Ok(i))) => match self.priv_check_incoming(i.into_data()) {
                Ok(Some(output)) => std::task::Poll::Ready(Some(Ok(output))),
                Ok(None) => std::task::Poll::Pending,
                Err(e) => std::task::Poll::Ready(Some(Err(e))),
            },
            std::task::Poll::Ready(Some(Err(e))) => {
                std::task::Poll::Ready(Some(Err(Error::new(ErrorKind::Other, e))))
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        };

        // finally clean up any expired pending requests
        self.priv_prune_pending();

        result
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
            WebsocketSender::priv_new(send_outgoing.clone()),
            Self {
                remote_addr,
                socket,
                send_outgoing,
                recv_outgoing,
                pending_requests: std::collections::HashMap::new(),
            },
        ))
    }

    /// internal helper for tracking request/response ids
    fn priv_prep_send(
        &mut self,
        msg: RpcMessage,
        respond: Option<tokio::sync::oneshot::Sender<Result<Vec<u8>>>>,
    ) -> Result<Vec<u8>> {
        let id = msg.clone_id();
        if respond.is_some() {
            self.pending_requests.insert(
                id,
                RequestItem {
                    expires_at: std::time::Instant::now()
                        .checked_add(std::time::Duration::from_millis(REQUEST_TIMEOUT_MS))
                        .expect("can set expires_at"),
                    respond,
                },
            );
        }
        let bytes: SerializedBytes = msg.try_into()?;
        let bytes: Vec<u8> = UnsafeBytes::from(bytes).into();
        Ok(bytes)
    }

    /// internal helper for processing incoming data
    fn priv_check_incoming(
        &mut self,
        msg: Vec<u8>,
    ) -> Result<Option<(SerializedBytes, WebsocketRespond)>> {
        let bytes: SerializedBytes = UnsafeBytes::from(msg).into();
        let msg: RpcMessage = bytes.try_into()?;
        match msg {
            RpcMessage::Request { id, data } => {
                //println!("RECEIVED REQ: {} {}", id, String::from_utf8_lossy(&data));
                // we got a request
                //  - set up a responder callback
                //  - notify our stream subscriber of the message
                let mut sender = self.send_outgoing.clone();
                let respond: WebsocketRespond = Box::new(|data| {
                    async move {
                        let msg = RpcMessage::Response {
                            id,
                            data: UnsafeBytes::from(data).into(),
                        };
                        sender
                            .send((msg, None))
                            .await
                            .map_err(|e| Error::new(ErrorKind::Other, e))?;
                        Ok(())
                    }
                    .boxed()
                });
                Ok(Some((UnsafeBytes::from(data).into(), respond)))
            }
            RpcMessage::Response { id, data } => {
                //println!("RECEIVED RES: {} {}", id, String::from_utf8_lossy(&data));
                // check our pending table / match up this response
                if let Some(mut item) = self.pending_requests.remove(&id) {
                    if let Some(respond) = item.respond.take() {
                        respond.send(Ok(data)).map_err(|_| {
                            Error::new(
                                ErrorKind::Other,
                                "oneshot channel closed - no one waiting on this response?",
                            )
                        })?;
                    }
                }
                Ok(None)
            }
        }
    }

    /// prune any expired pending responses
    fn priv_prune_pending(&mut self) {
        let now = std::time::Instant::now();
        self.pending_requests.retain(|_k, v| {
            if v.expires_at < now {
                if let Some(respond) = v.respond.take() {
                    let _ = respond.send(Err(ErrorKind::TimedOut.into()));
                }
                false
            } else {
                true
            }
        });
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
