//! We have two types of messages that must be dispatched
//!  - 1 - response registration requests from senders
//!  - 2 - actual incoming messages
//! This internal task manages both these cases.

use crate::*;
use task_socket_sink::ToSocketSinkSender;

/// internal message type for forwarding data to this task
#[derive(Debug)]
pub(crate) enum ToDispatchIncoming {
    RegisterResponse {
        id: String,
        respond: tokio::sync::oneshot::Sender<Result<SerializedBytes>>,
    },
    IncomingBytes(SerializedBytes),
    Close(WebsocketClosed),
}

/// internal ToDispatchIncoming Sender
pub(crate) type ToDispatchIncomingSender = tokio::sync::mpsc::Sender<ToDispatchIncoming>;

// /// internal ToDispatchIncoming Receiver
// pub(crate) type ToDispatchIncomingReceiver = tokio::sync::mpsc::Receiver<ToDispatchIncoming>;

/// See module-level documentation for this internal task
pub(crate) fn build(
    config: Arc<WebsocketConfig>,
    remote_addr: Url2,
    mut send_pub: ToWebsocketReceiverSender,
    mut send_sink: ToSocketSinkSender,
) -> ToDispatchIncomingSender {
    let (send_dispatch, mut recv_dispatch) = tokio::sync::mpsc::channel(config.max_send_queue);
    tokio::task::spawn(async move {
        tracing::trace!(
            message = "starting dispatch incoming task",
            %remote_addr,
        );
        let mut tracker = ResponseTracker::priv_new();
        use tokio::stream::StreamExt;
        while let Some(incoming) = recv_dispatch.next().await {
            match process_incoming_message(
                &config,
                &mut send_pub,
                &mut send_sink,
                &mut tracker,
                incoming,
            )
            .await
            {
                Ok(should_continue) => {
                    if !should_continue {
                        // end this task
                    }
                }
                Err(e) => {
                    tracing::error!(error = ?e);
                    let _ = send_pub.send(WebsocketMessage::Close(WebsocketClosed {
                        code: 0,
                        reason: format!("Internal Error: {:?}", e),
                    }));
                    // end this task
                    break;
                }
            }
            tracker.prune_expired();
        }
        tracing::info!(
            message = "dispatch incoming task ended",
            %remote_addr,
        );
    });
    send_dispatch
}

/// internal process a single incoming message
async fn process_incoming_message(
    config: &Arc<WebsocketConfig>,
    send_pub: &mut ToWebsocketReceiverSender,
    send_sink: &mut ToSocketSinkSender,
    tracker: &mut ResponseTracker,
    incoming: ToDispatchIncoming,
) -> Result<bool> {
    match incoming {
        // our sender half would like to register a callback for RPC response
        ToDispatchIncoming::RegisterResponse { id, respond } => {
            let item = ResponseItem {
                expires_at: std::time::Instant::now()
                    .checked_add(std::time::Duration::from_secs(
                        config.default_request_timeout_s as u64,
                    ))
                    .expect("can set expires_at"),
                respond: Some(respond),
                span: tracing::debug_span!("await_response"),
            };
            tracker.register_response(id, item);
        }
        // we have incoming data on the raw socket
        ToDispatchIncoming::IncomingBytes(bytes) => {
            let msg: WireMessage = bytes.try_into()?;
            match msg {
                WireMessage::Signal { data } => {
                    let data: SerializedBytes = UnsafeBytes::from(data).into();
                    tracing::trace!(message = "recieved signal", ?data,);
                    send_pub
                        .send(WebsocketMessage::Signal(data))
                        .await
                        .map_err(|e| Error::new(ErrorKind::Other, e))?;
                }
                WireMessage::Request { id, data } => {
                    let data: SerializedBytes = UnsafeBytes::from(data).into();
                    tracing::trace!(message = "recieved request", ?data,);
                    let mut loc_send_sink = send_sink.clone();
                    let respond: WebsocketRespond = Box::new(move |data| {
                        //let span = tracing::debug_span!("respond");
                        async move {
                            let msg = WireMessage::Response {
                                id,
                                data: UnsafeBytes::from(data).into(),
                            };
                            let bytes: SerializedBytes = msg.try_into()?;
                            let bytes: Vec<u8> = UnsafeBytes::from(bytes).into();

                            let msg = tungstenite::Message::Binary(bytes);
                            let (send, recv) = tokio::sync::oneshot::channel();
                            loc_send_sink
                                .send((msg, send))
                                .await
                                .map_err(|e| Error::new(ErrorKind::Other, e))?;
                            recv.await.map_err(|e| Error::new(ErrorKind::Other, e))?;
                            Ok(())
                        }
                        .boxed()
                    });
                    send_pub
                        .send(WebsocketMessage::Request(data, respond))
                        .await
                        .map_err(|e| Error::new(ErrorKind::Other, e))?;
                }
                WireMessage::Response { id, data } => {
                    let data: SerializedBytes = UnsafeBytes::from(data).into();
                    tracing::trace!(message = "recieved response", ?data,);
                    tracker.handle_response(id, data);
                }
            }
        }
        // our raw socket is closed
        ToDispatchIncoming::Close(closed) => {
            send_pub
                .send(WebsocketMessage::Close(closed))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            // end this task
            return Ok(false);
        }
    }

    // task can continue
    Ok(true)
}

/// internal track a response callback
struct ResponseItem {
    expires_at: std::time::Instant,
    respond: Option<tokio::sync::oneshot::Sender<Result<SerializedBytes>>>,
    span: tracing::Span,
}

/// internal struct for tracking response callbacks
struct ResponseTracker {
    pending_responses: std::collections::HashMap<String, ResponseItem>,
}

impl ResponseTracker {
    fn priv_new() -> Self {
        Self {
            pending_responses: std::collections::HashMap::new(),
        }
    }

    /// register a response item to be invoked later if we get that response
    fn register_response(&mut self, id: String, item: ResponseItem) {
        self.pending_responses.insert(id, item);
    }

    /// we received a response, try to match it up to a pending callback
    fn handle_response(&mut self, id: String, data: SerializedBytes) {
        if let Some(mut item) = self.pending_responses.remove(&id) {
            let _g = item.span.enter();
            if let Some(respond) = item.respond.take() {
                if let Err(e) = respond.send(Ok(data)) {
                    tracing::warn!(error = ?e);
                }
            }
        }
    }

    /// check for any expired response callbacks - trigger timeout errors
    fn prune_expired(&mut self) {
        let now = std::time::Instant::now();
        self.pending_responses.retain(|_k, v| {
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
