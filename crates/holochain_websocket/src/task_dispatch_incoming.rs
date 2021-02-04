//! We have two types of messages that must be dispatched
//!  - 1 - response registration requests from senders
//!  - 2 - actual incoming messages
//! This internal task manages both these cases.

use super::task_socket_sink::ToSocketSinkSender;
use crate::*;

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
                        break;
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
                _span: tracing::Span::none(),
            };
            tracker.register_response(id, item);
        }
        // we have incoming data on the raw socket
        ToDispatchIncoming::IncomingBytes(bytes) => {
            let msg: WireMessage = bytes.try_into()?;
            match msg {
                WireMessage::Signal { data } => {
                    let data: SerializedBytes = UnsafeBytes::from(data).into();
                    tracing::trace!(message = "received signal", ?data,);
                    send_pub
                        .send(WebsocketMessage::Signal(data))
                        .await
                        .map_err(|e| Error::new(ErrorKind::Other, e))?;
                }
                WireMessage::Request { id, data } => {
                    let data: SerializedBytes = UnsafeBytes::from(data).into();
                    tracing::trace!(message = "received request", ?data,);
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
                    tracing::trace!(message = "received response", ?data,);
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
    _span: tracing::Span,
}

/// internal struct for tracking response callbacks
struct ResponseTracker {
    pending_responses: std::collections::HashMap<String, ResponseItem>,
}

impl ResponseTracker {
    /// internal constructor
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::stream::StreamExt;

    struct Prep {
        recv_pub: tokio::sync::mpsc::Receiver<WebsocketMessage>,
        recv_sink:
            tokio::sync::mpsc::Receiver<(tungstenite::Message, tokio::sync::oneshot::Sender<()>)>,
        send_dispatch: ToDispatchIncomingSender,
    }

    fn prep_test() -> Prep {
        let (send_pub, recv_pub) = tokio::sync::mpsc::channel(1);
        let (send_sink, recv_sink) = tokio::sync::mpsc::channel(1);

        let send_dispatch = build(
            Arc::new(WebsocketConfig::default().default_request_timeout_s(1)),
            url2!("test://"),
            send_pub,
            send_sink,
        );

        Prep {
            recv_pub,
            recv_sink,
            send_dispatch,
        }
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct TestMessage(String);
    try_from_serialized_bytes!(TestMessage);

    fn test_signal(s: &str) -> ToDispatchIncoming {
        let data: SerializedBytes = TestMessage(s.to_string()).try_into().unwrap();
        let data: Vec<u8> = UnsafeBytes::from(data).into();

        let msg = WireMessage::Signal { data };
        let data: SerializedBytes = msg.try_into().unwrap();
        ToDispatchIncoming::IncomingBytes(data)
    }

    fn test_request(s: &str) -> ToDispatchIncoming {
        let data: SerializedBytes = TestMessage(s.to_string()).try_into().unwrap();
        let data: Vec<u8> = UnsafeBytes::from(data).into();

        let msg = WireMessage::Request {
            id: nanoid::nanoid!(),
            data,
        };
        let data: SerializedBytes = msg.try_into().unwrap();
        ToDispatchIncoming::IncomingBytes(data)
    }

    fn test_register_response() -> (
        String,
        ToDispatchIncoming,
        tokio::sync::oneshot::Receiver<Result<SerializedBytes>>,
    ) {
        let (respond, recv) = tokio::sync::oneshot::channel();
        let id = nanoid::nanoid!();
        (
            id.clone(),
            ToDispatchIncoming::RegisterResponse { id, respond },
            recv,
        )
    }

    fn test_response(id: String, s: &str) -> ToDispatchIncoming {
        let data: SerializedBytes = TestMessage(s.to_string()).try_into().unwrap();
        let data: Vec<u8> = UnsafeBytes::from(data).into();

        let msg = WireMessage::Response { id, data };
        let data: SerializedBytes = msg.try_into().unwrap();
        ToDispatchIncoming::IncomingBytes(data)
    }

    fn test_close() -> ToDispatchIncoming {
        ToDispatchIncoming::Close(WebsocketClosed {
            code: 42,
            reason: "test-reason".to_string(),
        })
    }

    #[tokio::test]
    async fn test_task_dispatch_incoming() {
        init_tracing();

        let Prep {
            mut recv_pub,
            mut recv_sink,
            mut send_dispatch,
        } = prep_test();

        let (mut my_sink_send, mut my_sink_recv) = tokio::sync::mpsc::channel(1);

        tokio::task::spawn(async move {
            while let Some((msg, complete)) = recv_sink.next().await {
                let msg: WireMessage = match msg {
                    tungstenite::Message::Binary(msg) => {
                        let msg: SerializedBytes = UnsafeBytes::from(msg).into();
                        msg.try_into().unwrap()
                    }
                    _ => panic!("unexpected tungstenite type"),
                };
                my_sink_send.send(msg).await.unwrap();
                complete.send(()).unwrap();
            }
        });

        send_dispatch.send(test_signal("test1")).await.unwrap();

        assert_eq!(
            "WebsocketMessage::Signal { bytes: 6 }",
            &format!("{:?}", recv_pub.next().await.unwrap()),
        );

        send_dispatch.send(test_request("test2")).await.unwrap();

        let (msg, respond) = match recv_pub.next().await.unwrap() {
            WebsocketMessage::Request(msg, respond) => (msg, respond),
            _ => panic!("unexpected recv_pub type"),
        };
        let msg: TestMessage = msg.try_into().unwrap();

        assert_eq!("test2", &msg.0);

        respond(TestMessage("test3".to_string()).try_into().unwrap())
            .await
            .unwrap();

        match my_sink_recv.next().await.unwrap() {
            WireMessage::Response { id: _, data } => {
                let msg: SerializedBytes = UnsafeBytes::from(data).into();
                let msg: TestMessage = msg.try_into().unwrap();
                assert_eq!("test3", &msg.0,);
            }
            _ => panic!("unexpected response"),
        }

        let (id, msg, recv) = test_register_response();

        send_dispatch.send(msg).await.unwrap();

        send_dispatch
            .send(test_response(id, "test4"))
            .await
            .unwrap();

        assert_eq!("Ok(\"test4\")", &format!("{:?}", recv.await.unwrap()));

        send_dispatch.send(test_close()).await.unwrap();

        assert_eq!(
            "WebsocketMessage::Close { close: WebsocketClosed { code: 42, reason: \"test-reason\" } }",
            &format!("{:?}", recv_pub.next().await.unwrap()),
        );

        assert_eq!("None", &format!("{:?}", recv_pub.next().await));

        assert_eq!("None", &format!("{:?}", my_sink_recv.next().await));
    }
}
