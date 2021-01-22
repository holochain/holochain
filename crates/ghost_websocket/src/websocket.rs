use std::collections::VecDeque;
use std::convert::TryInto;
use std::sync::Arc;

use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
use holochain_serialized_bytes::prelude::*;
use stream_cancel::Trigger;
use stream_cancel::Valved;
use tracing::instrument;
use tracing::Instrument;

use ghost_actor::*;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::CloseFrame;

use crate::util::ToFromSocket;
use crate::IncomingMessage;
use crate::OutgoingMessage;
use crate::RegisterResponse;
use crate::Response;
use crate::WebsocketConfig;
use crate::WebsocketError;
use crate::WebsocketReceiver;
use crate::WebsocketSender;

type GhostResult<T> = std::result::Result<T, GhostError>;

#[derive(Debug, Clone)]
pub struct Websocket(GhostActor<WebsocketInner>);

#[derive(Debug)]
struct ResponseTracker {
    responses: Vec<Option<RegisterResponse>>,
    free_indices: VecDeque<usize>,
}

impl ResponseTracker {
    fn new() -> Self {
        Self {
            responses: Vec::new(),
            free_indices: VecDeque::new(),
        }
    }
}

struct WebsocketInner {
    responses: ResponseTracker,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
enum WireMessage {
    Signal(SerializedBytes),
    Request(SerializedBytes, u32),
    Response(SerializedBytes, u32),
}

// Channel to the websocket
pub(crate) type TxToWebsocket = tokio::sync::mpsc::Sender<OutgoingMessage>;
type RxToWebsocket = tokio::sync::mpsc::Receiver<OutgoingMessage>;

// Channel from the websocket
pub(crate) type TxFromWebsocket = tokio::sync::mpsc::Sender<IncomingMessage>;
pub type RxFromWebsocket = tokio::sync::mpsc::Receiver<IncomingMessage>;

impl Websocket {
    #[instrument(skip(config, socket))]
    pub fn create_ends(
        config: Arc<WebsocketConfig>,
        socket: ToFromSocket,
    ) -> (WebsocketSender, WebsocketReceiver) {
        let (tx_to_websocket, rx_to_websocket) = tokio::sync::mpsc::channel(config.max_send_queue);
        let (tx_from_websocket, rx_from_websocket) =
            tokio::sync::mpsc::channel(config.max_send_queue);
        Websocket::new(
            socket,
            tx_to_websocket.clone(),
            rx_to_websocket,
            tx_from_websocket,
        );
        let sender = WebsocketSender::new(tx_to_websocket);
        let receiver = WebsocketReceiver::new(rx_from_websocket);
        (sender, receiver)
    }

    #[instrument(skip(socket, tx_to_websocket, rx_to_websocket, tx_from_websocket))]
    fn new(
        socket: ToFromSocket,
        tx_to_websocket: TxToWebsocket,
        rx_to_websocket: RxToWebsocket,
        tx_from_websocket: TxFromWebsocket,
    ) -> Self {
        let (actor, driver) = GhostActor::new(WebsocketInner {
            responses: ResponseTracker::new(),
        });
        tokio::task::spawn(driver);
        Self::run_socket(
            actor.clone(),
            socket,
            tx_to_websocket,
            rx_to_websocket,
            tx_from_websocket,
        );

        Self(actor)
    }

    fn run_socket(
        actor: GhostActor<WebsocketInner>,
        socket: ToFromSocket,
        send_response: TxToWebsocket,
        inbound: RxToWebsocket,
        outbound: TxFromWebsocket,
    ) {
        let (to_socket, from_socket) = socket.split();
        let (shutdown_from_socket, from_socket) = Valved::new(from_socket);
        let (shutdown_to_socket, inbound) = Valved::new(inbound);
        tokio::task::spawn(
            Self::run_to_socket(actor.clone(), to_socket, inbound, shutdown_from_socket)
                .in_current_span(),
        );
        tokio::task::spawn(
            Self::run_from_socket(
                actor,
                from_socket,
                outbound,
                send_response,
                shutdown_to_socket,
            )
            .in_current_span(),
        );
    }

    #[instrument(skip(actor, to_socket, inbound, _shutdown_from_socket))]
    async fn run_to_socket(
        actor: GhostActor<WebsocketInner>,
        to_socket: impl futures::sink::Sink<tungstenite::Message, Error = tungstenite::error::Error>,
        mut inbound: Valved<RxToWebsocket>,
        _shutdown_from_socket: Trigger,
    ) {
        tracing::trace!("starting sending external socket");
        futures::pin_mut!(to_socket);
        'send_loop: loop {
            match inbound.next().await {
                Some(msg) => {
                    tracing::trace!(sending_msg = ?msg);
                    let msg = match msg {
                        OutgoingMessage::Signal(msg) => WireMessage::Signal(msg),
                        OutgoingMessage::Request(msg, register_response) => {
                            // register outgoing message
                            if !actor.is_active() {
                                tracing::error!("Actor is closed");
                                // TODO: Send close
                                break 'send_loop;
                            }
                            let id = match actor
                                .invoke(move |state| {
                                    GhostResult::Ok(state.responses.register(register_response))
                                })
                                .await
                            {
                                Ok(id) => id,
                                Err(e) => {
                                    tracing::error!(?e);
                                    // TODO: Send close
                                    break 'send_loop;
                                }
                            };
                            tracing::debug!(?id);
                            WireMessage::Request(msg, id)
                        }
                        OutgoingMessage::Response(msg, id) => WireMessage::Response(msg, id),
                    };
                    let msg: SerializedBytes = match msg.try_into() {
                        Ok(msg) => msg,
                        Err(e) => {
                            tracing::error!("Websocket: Message failed to serialize {:?}", e);
                            // Should not kill the websocket just because a single message
                            // failed serialization.
                            continue 'send_loop;
                        }
                    };
                    let bytes: Vec<u8> = UnsafeBytes::from(msg).into();

                    let msg = tungstenite::Message::Binary(bytes);
                    // Write to_socket
                    if let Err(e) = to_socket.send(msg).await {
                        // TODO: If write fails then close both connections
                        tracing::error!(?e);
                        break 'send_loop;
                    }
                    // TODO: If request track on id on hashmap
                    // TODO: Parse message to correct format
                }
                None => {
                    if let Err(e) = to_socket
                        .send(tungstenite::Message::Close(Some(CloseFrame {
                            code: CloseCode::Normal,
                            reason: "Shutting down sender".into(),
                        })))
                        .await
                    {
                        tracing::error!(msg = "Failed to send close from from sender", ?e);
                    } else {
                        tracing::debug!("sent closing frame");
                    }
                    // TODO: Shutdown the receiver with a valve
                    break 'send_loop;
                }
            }
        }
        // TODO: Just send close on break here
        tracing::trace!("exiting sending to external socket");
    }

    #[instrument(skip(actor, from_socket, outbound, send_response, _shutdown_to_socket))]
    async fn run_from_socket(
        actor: GhostActor<WebsocketInner>,
        from_socket: impl futures::stream::Stream<
            Item = std::result::Result<tungstenite::Message, tungstenite::error::Error>,
        >,
        mut outbound: TxFromWebsocket,
        send_response: TxToWebsocket,
        _shutdown_to_socket: Trigger,
    ) {
        tracing::trace!("starting receiving from external socket");
        futures::pin_mut!(from_socket);
        'recv_loop: loop {
            match from_socket.next().await {
                Some(Ok(msg)) => {
                    tracing::debug!(?msg);
                    match msg {
                        tungstenite::Message::Binary(bytes) => {
                            let msg: WireMessage =
                                match SerializedBytes::try_from(UnsafeBytes::from(bytes))
                                    .map_err(|e| WebsocketError::from(e))
                                    .and_then(|sb| Ok(WireMessage::try_from(sb)?))
                                {
                                    Ok(msg) => msg,
                                    Err(e) => {
                                        tracing::error!("Websocket failed to deserialize {:?}", e,);
                                        continue 'recv_loop;
                                    }
                                };
                            let (msg, resp) = match msg {
                                WireMessage::Signal(msg) => {
                                    let no_op = |_| async move { Ok(()) }.boxed().into();
                                    let resp: Response = Box::new(no_op);
                                    (msg, resp)
                                }
                                WireMessage::Request(msg, id) => {
                                    let resp = {
                                        let mut send_response = send_response.clone();
                                        move |msg| {
                                            async move {
                                                let msg = OutgoingMessage::Response(msg, id);
                                                send_response.send(msg).await.map_err(|_| {
                                                    WebsocketError::FailedToSendResp
                                                })?;

                                                Ok(())
                                            }
                                            .boxed()
                                            .into()
                                        }
                                    };
                                    let resp: Response = Box::new(resp);
                                    (msg, resp)
                                }
                                WireMessage::Response(msg, id) => {
                                    if !actor.is_active() {
                                        tracing::error!("Actor is closed");
                                        break 'recv_loop;
                                    }
                                    match actor
                                        .invoke(move |state| {
                                            GhostResult::Ok(state.responses.pop(id))
                                        })
                                        .await
                                        .map_err(WebsocketError::from)
                                        .and_then(|response| match response {
                                            Some(r) => r.respond(msg),
                                            None => {
                                                tracing::error!("Websocket: Received response for request that doesn't exist");
                                                Ok(())
                                            }
                                        })
                                    {
                                        Ok(_) => {
                                            continue 'recv_loop;
                                        }
                                        Err(e) => {
                                            tracing::error!(?e);
                                            break 'recv_loop;
                                        }
                                    }
                                }
                            };
                            if let Err(_) = outbound.send((msg, resp)).await {
                                // TODO: Send close frame
                                // TODO: Shutdown send loop
                                break 'recv_loop;
                            }
                        }
                        tungstenite::Message::Close(_) => {
                            break 'recv_loop;
                        }
                        _ => {
                            tracing::error!("Bad message type");
                        }
                    }
                }
                Some(Err(e)) => {
                    tracing::error!(?e);
                    break 'recv_loop;
                }
                None => {
                    break 'recv_loop;
                }
            }
        }
        tracing::trace!("exiting receiving from external socket");
    }
}

impl ResponseTracker {
    fn register(&mut self, response: RegisterResponse) -> u32 {
        match self
            .free_indices
            .pop_front()
            .map(|i| self.responses.get_mut(i).map(|e| (e, i as u32)))
        {
            Some(Some((empty, i))) => {
                *empty = Some(response);
                i
            }
            None | Some(None) => {
                let i = self.responses.len();
                self.responses.push(Some(response));
                i as u32
            }
        }
    }
    fn pop(&mut self, id: u32) -> Option<RegisterResponse> {
        let index = id as usize;
        let r = self.responses.get_mut(index).and_then(|slot| slot.take());
        if r.is_some() {
            self.free_indices.push_back(index);
        }
        r
    }
}
