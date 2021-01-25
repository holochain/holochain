use std::collections::VecDeque;
use std::convert::TryInto;
use std::sync::Arc;

use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
use holochain_serialized_bytes::prelude::*;
use stream_cancel::Trigger;
use stream_cancel::Valve;
use stream_cancel::Valved;
use tracing::instrument;
use tracing::Instrument;

use ghost_actor::*;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::CloseFrame;

use crate::util::ToFromSocket;
use crate::util::CLOSE_TIMEOUT;
use crate::IncomingMessage;
use crate::OutgoingMessage;
use crate::RegisterResponse;
use crate::Respond;
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
pub(crate) type RxFromWebsocket = tokio::sync::mpsc::Receiver<IncomingMessage>;

#[derive(Debug)]
pub struct PairShutdown {
    close_from_socket: Trigger,
    close_to_socket: TxToWebsocket,
}

type Loop<T> = std::result::Result<T, Task>;

#[derive(Clone, Copy)]
enum Task {
    Continue,
    Exit,
    ExitNow,
}

impl Task {
    fn cont<T>() -> Loop<T> {
        Err(Task::Continue)
    }

    /// Exit and allow channels to empty
    fn exit<T>() -> Loop<T> {
        Err(Task::Exit)
    }

    /// Exit immediately with emptying channels
    fn exit_now<T>() -> Loop<T> {
        Err(Task::Exit)
    }
}

impl Websocket {
    #[instrument(skip(config, socket, listener_shutdown))]
    pub fn create_ends(
        config: Arc<WebsocketConfig>,
        socket: ToFromSocket,
        listener_shutdown: Valve,
    ) -> (WebsocketSender, WebsocketReceiver) {
        let (tx_to_websocket, rx_to_websocket) = tokio::sync::mpsc::channel(config.max_send_queue);
        let (tx_from_websocket, rx_from_websocket) =
            tokio::sync::mpsc::channel(config.max_send_queue);

        // If both handles are dropped then we want to shutdown the to/from socket tasks
        let (close_from_socket, pair_shutdown) = Valve::new();
        let pair_shutdown_handle = PairShutdown {
            close_to_socket: tx_to_websocket.clone(),
            close_from_socket,
        };
        // Only shutdown if both trigger arcs are dropped
        let pair_shutdown_handle = Arc::new(pair_shutdown_handle);

        Websocket::run(
            socket,
            tx_to_websocket.clone(),
            rx_to_websocket,
            tx_from_websocket,
            pair_shutdown,
        );

        // Shutdown the receiver stream if the listener is dropped
        let rx_from_websocket = listener_shutdown.wrap(rx_from_websocket);

        let sender = WebsocketSender::new(
            tx_to_websocket,
            listener_shutdown,
            pair_shutdown_handle.clone(),
        );
        let receiver = WebsocketReceiver::new(rx_from_websocket, pair_shutdown_handle);
        (sender, receiver)
    }

    #[instrument(skip(
        socket,
        tx_to_websocket,
        rx_to_websocket,
        tx_from_websocket,
        pair_shutdown
    ))]
    fn run(
        socket: ToFromSocket,
        tx_to_websocket: TxToWebsocket,
        rx_to_websocket: RxToWebsocket,
        tx_from_websocket: TxFromWebsocket,
        pair_shutdown: Valve,
    ) {
        let (actor, driver) = GhostActor::new(WebsocketInner {
            responses: ResponseTracker::new(),
        });
        tokio::task::spawn(driver);
        let actor = Self(actor);
        actor.run_socket(
            socket,
            tx_to_websocket,
            rx_to_websocket,
            tx_from_websocket,
            pair_shutdown,
        );
    }

    fn run_socket(
        self,
        socket: ToFromSocket,
        send_response: TxToWebsocket,
        to_websocket: RxToWebsocket,
        from_websocket: TxFromWebsocket,
        pair_shutdown: Valve,
    ) {
        let (to_socket, from_socket) = socket.split();
        let (shutdown_from_socket, from_socket) = Valved::new(from_socket);
        let from_socket = pair_shutdown.wrap(from_socket);
        let (shutdown_to_socket, to_websocket) = Valved::new(to_websocket);
        tokio::task::spawn(
            self.clone()
                .run_to_socket(to_socket, to_websocket, shutdown_from_socket)
                .in_current_span(),
        );
        tokio::task::spawn(
            self.run_from_socket(
                from_socket,
                from_websocket,
                send_response,
                shutdown_to_socket,
            )
            .in_current_span(),
        );
    }

    #[instrument(skip(self, to_socket, to_websocket, _shutdown_from_socket))]
    /// Task that sends out messages to the network.
    async fn run_to_socket(
        self,
        to_socket: impl futures::sink::Sink<tungstenite::Message, Error = tungstenite::error::Error>,
        mut to_websocket: Valved<RxToWebsocket>,
        // When dropped this will shutdown the `from_socket` task.
        _shutdown_from_socket: Trigger,
    ) {
        tracing::trace!("starting sending external socket");
        futures::pin_mut!(to_socket);
        'send_loop: loop {
            if let Err(Task::Exit) = self
                .process_to_websocket(to_websocket.next().await, &mut to_socket)
                .await
            {
                break 'send_loop;
            }
        }
        // Send close frame. If it fails to send there's
        // not much we can do.
        to_socket
            .send(tungstenite::Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: "Shutting down sender".into(),
            })))
            .await
            .ok();
        tracing::trace!("exiting sending to external socket");
    }

    /// Process messages coming from the application to
    /// the websocket actor and pass them onto the network.
    async fn process_to_websocket(
        &self,
        msg: Option<OutgoingMessage>,
        to_socket: &mut std::pin::Pin<
            &mut impl futures::sink::Sink<tungstenite::Message, Error = tungstenite::error::Error>,
        >,
    ) -> Loop<()> {
        match msg {
            Some(msg) => {
                tracing::trace!(sending_msg = ?msg);
                let msg = match msg {
                    OutgoingMessage::Close => return Task::exit(),
                    OutgoingMessage::Signal(msg) => WireMessage::Signal(msg),
                    OutgoingMessage::Request(msg, register_response) => {
                        self.handle_outgoing_request(msg, register_response).await?
                    }
                    OutgoingMessage::Response(msg, id) => WireMessage::Response(msg, id),
                    OutgoingMessage::Pong(data) => {
                        to_socket.send(tungstenite::Message::Pong(data)).await.ok();
                        return Task::cont();
                    }
                };
                let msg = Self::serialize_msg(msg)?;
                // Write to_socket
                match to_socket.send(msg).await {
                    Ok(_) => Task::cont(),
                    Err(tungstenite::Error::ConnectionClosed) => Task::exit(),
                    Err(e) => {
                        // If write fails then close both connections
                        tracing::error!(?e);
                        Task::exit()
                    }
                }
            }
            None => Task::exit(),
        }
    }

    #[instrument(skip(
        self,
        from_socket,
        from_websocket,
        send_response,
        shutdown_from_socket_immediately
    ))]
    /// Task that takes in messages from the network.
    async fn run_from_socket(
        self,
        from_socket: impl futures::stream::Stream<
            Item = std::result::Result<tungstenite::Message, tungstenite::Error>,
        >,
        mut from_websocket: TxFromWebsocket,
        mut send_response: TxToWebsocket,
        shutdown_from_socket_immediately: Trigger,
    ) {
        tracing::trace!("starting receiving from external socket");
        futures::pin_mut!(from_socket);
        let mut task = Task::Continue;
        'recv_loop: loop {
            let msg = from_socket.next().await;
            if let Err(t) = self
                .process_from_websocket(msg, &mut from_websocket, &mut send_response)
                .await
            {
                task = t;
            }
            if let Task::Exit | Task::ExitNow = task {
                break 'recv_loop;
            }
        }
        // Try a graceful shutdown
        if let Task::Exit = task {
            if send_response
                .send_timeout(OutgoingMessage::Close, CLOSE_TIMEOUT)
                .await
                .is_ok()
            {
                shutdown_from_socket_immediately.disable();
            }
        }
        tracing::trace!("exiting receiving from external socket");
    }

    /// Process messages coming from the network and pass
    /// them onto the `FromWebsocket` channel.
    async fn process_from_websocket(
        &self,
        msg: Option<std::result::Result<tungstenite::Message, tungstenite::Error>>,
        from_websocket: &mut TxFromWebsocket,
        send_response: &mut TxToWebsocket,
    ) -> Loop<()> {
        match msg {
            Some(Ok(msg)) => {
                tracing::trace!(received_msg = ?msg);
                match msg {
                    tungstenite::Message::Binary(bytes) => {
                        let msg = Self::deserialize_message(bytes)?;
                        let (msg, resp) = match msg {
                            WireMessage::Signal(msg) => (msg, Respond::Signal),
                            WireMessage::Request(msg, id) => {
                                Self::handle_incoming_request(send_response, msg, id)
                            }
                            WireMessage::Response(msg, id) => {
                                return self.handle_incoming_response(msg, id).await;
                            }
                        };
                        if from_websocket
                            .send(IncomingMessage::Msg(msg, resp))
                            .await
                            .is_err()
                        {
                            Task::exit()
                        } else {
                            Task::cont()
                        }
                    }
                    tungstenite::Message::Close(_) => {
                        // Send a close command to the websocket receiver
                        // and wait for acknowledgment so that the receiver
                        // can process any messages still in the queue.
                        let (acknowledge, resp) = tokio::sync::oneshot::channel();
                        if from_websocket
                            .send_timeout(IncomingMessage::Close { acknowledge }, CLOSE_TIMEOUT)
                            .await
                            .is_ok()
                        {
                            tokio::time::timeout(CLOSE_TIMEOUT, resp).await.ok();
                        }
                        Task::exit_now()
                    }
                    tungstenite::Message::Ping(data) => {
                        send_response.send(OutgoingMessage::Pong(data)).await.ok();
                        Task::cont()
                    }
                    m => {
                        tracing::error!("Websocket: Bad message type {:?}", m);
                        Task::cont()
                    }
                }
            }
            Some(Err(e)) => {
                tracing::error!(error_from_incoming_websocket = ?e);
                Task::exit_now()
            }
            None => Task::exit(),
        }
    }

    /// Handling a request coming in from the network
    /// and reply with a response.
    fn handle_incoming_request(
        send_response: &mut TxToWebsocket,
        msg: SerializedBytes,
        id: u32,
    ) -> (SerializedBytes, Respond) {
        let resp = {
            let mut send_response = send_response.clone();

            // Callback to respond to the request
            move |msg| {
                async move {
                    let msg = OutgoingMessage::Response(msg, id);

                    // Send the response to the to_socket task
                    send_response
                        .send(msg)
                        .await
                        .map_err(|_| WebsocketError::FailedToSendResp)?;
                    tracing::trace!("Sent response");

                    Ok(())
                }
                .boxed()
                .into()
            }
        };
        let resp = Respond::Request(Box::new(resp));
        (msg, resp)
    }

    /// Handle a requesting going out to the network.
    async fn handle_outgoing_request(
        &self,
        msg: SerializedBytes,
        register_response: RegisterResponse,
    ) -> Loop<WireMessage> {
        // register outgoing message
        if !self.0.is_active() {
            tracing::error!("Actor is closed");
            return Task::exit();
        }
        let id = match self
            .0
            .invoke(move |state| GhostResult::Ok(state.responses.register(register_response)))
            .await
        {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(?e);
                return Task::exit();
            }
        };
        Ok(WireMessage::Request(msg, id))
    }

    /// Handle a response coming in from the network.
    async fn handle_incoming_response(&self, msg: SerializedBytes, id: u32) -> Loop<()> {
        if !self.0.is_active() {
            tracing::error!("Actor is closed");
            return Task::exit();
        }
        let r = self
            .0
            .invoke(move |state| GhostResult::Ok(state.responses.pop(id)))
            .await
            .map_err(WebsocketError::from)
            .and_then(|response| match response {
                Some(r) => r.respond(msg),
                None => {
                    tracing::error!("Websocket: Received response for request that doesn't exist");
                    Ok(())
                }
            });

        match r {
            Ok(_) => {
                return Task::cont();
            }
            Err(e) => {
                tracing::error!(?e);
                return Task::exit();
            }
        }
    }

    /// Try to serialize the wire message and continue to next
    /// message if failure.
    fn serialize_msg(msg: WireMessage) -> Loop<tungstenite::Message> {
        let msg: SerializedBytes = match msg.try_into() {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!("Websocket: Message failed to serialize {:?}", e);
                // Should not kill the websocket just because a single message
                // failed serialization.
                return Task::cont();
            }
        };
        let bytes: Vec<u8> = UnsafeBytes::from(msg).into();

        let msg = tungstenite::Message::Binary(bytes);
        Ok(msg)
    }

    /// Try to deserialize the wire message and continue to next
    /// message if failure.
    fn deserialize_message(bytes: Vec<u8>) -> Loop<WireMessage> {
        match SerializedBytes::try_from(UnsafeBytes::from(bytes))
            .map_err(WebsocketError::from)
            .and_then(|sb| Ok(WireMessage::try_from(sb)?))
        {
            Ok(msg) => Ok(msg),
            Err(e) => {
                tracing::error!("Websocket failed to deserialize {:?}", e,);
                // Should not kill the websocket just because a single message
                // failed serialization.
                Task::cont()
            }
        }
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

impl Drop for PairShutdown {
    fn drop(&mut self) {
        // Try to send a close to the "to socket task"
        self.close_to_socket.try_send(OutgoingMessage::Close).ok();
    }
}
