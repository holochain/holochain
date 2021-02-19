use std::collections::HashMap;
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

use crate::util::addr_to_url;
use crate::util::ToFromSocket;
use crate::util::CLOSE_TIMEOUT;
use crate::CancelResponse;
use crate::IncomingMessage;
use crate::OutgoingMessage;
use crate::RegisterResponse;
use crate::Respond;
use crate::TxRequestsDebug;
use crate::TxStaleRequest;
use crate::WebsocketConfig;
use crate::WebsocketError;
use crate::WebsocketReceiver;
use crate::WebsocketResult;
use crate::WebsocketSender;
use crate::WireMessage;

type GhostResult<T> = std::result::Result<T, GhostError>;

#[derive(Debug, Clone)]
/// Actor that tracks responses.
pub struct Websocket(GhostActor<WebsocketInner>);

#[derive(Debug)]
struct ResponseTracker {
    /// Map of registered responses.
    responses: HashMap<u64, RegisterResponse>,
    /// The next key to use.
    index: u64,
}

/// Inner GhostActor data.
struct WebsocketInner {
    responses: ResponseTracker,
}

// Channel from the application to the websocket and out to the external socket.

/// Send from application to the websocket.
pub(crate) type TxToWebsocket = tokio::sync::mpsc::Sender<OutgoingMessage>;
/// Receive in the websocket from the application.
type RxToWebsocket = tokio::sync::mpsc::Receiver<OutgoingMessage>;

// Channel from external socket then from the websocket to the application.

/// Send from the websocket to the application.
pub(crate) type TxFromWebsocket = tokio::sync::mpsc::Sender<IncomingMessage>;
/// Receive in the application from the websocket.
pub(crate) type RxFromWebsocket = tokio::sync::mpsc::Receiver<IncomingMessage>;

#[derive(Debug)]
/// When dropped both to / from socket tasks are shutdown.
pub struct PairShutdown {
    close_from_socket: Trigger,
    close_to_socket: TxToWebsocket,
}

/// Allows returning from inner functions with
/// success (continue to next line not continue loop),
/// continue (continue the loop), break for the outer task loop.
type Loop<T> = std::result::Result<T, Task>;

#[derive(Clone, Copy)]
enum Task {
    /// Same as
    /// ```no_run
    /// loop {
    ///   continue;
    /// }
    /// ```
    Continue,
    /// Same as
    /// ```no_run
    /// loop {
    ///   break;
    /// }
    /// ```
    Exit,
    /// Same as exit but skips sending
    /// a websocket close message.
    /// This happens when something fails that
    /// would prevent any graceful shutdown.
    ExitNow,
}

impl Task {
    /// Continue the loop.
    fn cont<T>() -> Loop<T> {
        Err(Task::Continue)
    }

    /// Exit and allow channels to empty.
    fn exit<T>() -> Loop<T> {
        Err(Task::Exit)
    }

    /// Exit immediately with emptying channels.
    fn exit_now<T>() -> Loop<T> {
        Err(Task::Exit)
    }
}

impl Websocket {
    #[instrument(skip(config, socket, listener_shutdown))]
    /// Create the ends of this websocket channel.
    pub fn create_ends(
        config: Arc<WebsocketConfig>,
        socket: ToFromSocket,
        listener_shutdown: Valve,
    ) -> WebsocketResult<(WebsocketSender, WebsocketReceiver)> {
        let remote_addr = url2::url2!(
            "{}#{}",
            addr_to_url(socket.get_ref().peer_addr()?, config.scheme),
            nanoid::nanoid!(),
        );

        // Channel to the websocket from the application
        let (tx_to_websocket, rx_to_websocket) = tokio::sync::mpsc::channel(config.max_send_queue);
        // Channel from the websocket to the application
        let (tx_from_websocket, rx_from_websocket) =
            tokio::sync::mpsc::channel(config.max_send_queue);

        // ---- PAIR SHUTDOWN ---- //
        // If both channel ends are dropped then we want to shutdown the to/from socket tasks
        let (close_from_socket, pair_shutdown) = Valve::new();
        let pair_shutdown_handle = PairShutdown {
            close_to_socket: tx_to_websocket.clone(),
            close_from_socket,
        };
        // Only shutdown if both trigger arcs are dropped
        let pair_shutdown_handle = Arc::new(pair_shutdown_handle);

        // ---- LISTENER SHUTDOWN ---- //

        // Shutdown the receiver stream if the listener is dropped
        // TODO: Should this shutdown immediately or gracefully. Currently it is immediately.
        let rx_from_websocket = listener_shutdown.wrap(rx_from_websocket);

        // Run the to and from external socket tasks.
        Websocket::run(
            socket,
            tx_to_websocket.clone(),
            rx_to_websocket,
            tx_from_websocket,
            pair_shutdown,
        );

        // Create the sender end.
        let sender = WebsocketSender::new(
            tx_to_websocket,
            listener_shutdown,
            pair_shutdown_handle.clone(),
        );
        // Create the receiver end.
        let receiver = WebsocketReceiver::new(rx_from_websocket, remote_addr, pair_shutdown_handle);
        Ok((sender, receiver))
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
        // Spawn the actor and run the socket tasks
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
        // Get the ends to the external socket.
        let (to_socket, from_socket) = socket.split();

        // ---- TASK SHUTDOWN ---- //
        // These cause immediate shutdown:
        // - Shutdown from_socket task because to_socket task has shutdown.
        let (shutdown_from_socket, from_socket) = Valved::new(from_socket);
        // - Shutdown from_socket task because both channel ends have dropped.
        // PairShutdown will also send a close message to to_socket.
        let from_socket = pair_shutdown.wrap(from_socket);
        // - Shutdown to_socket task because from_socket task has shutdown.
        // This valve will not close is to_socket can successfully send a close message to from_socket.
        let (shutdown_to_socket, to_websocket) = Valved::new(to_websocket);

        // Spawn the "to" external task.
        tokio::task::spawn(
            self.clone()
                .run_to_socket(to_socket, to_websocket, shutdown_from_socket)
                .in_current_span(),
        );
        // Spawn the "from" external task.
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
        let mut task = Task::Continue;
        tracing::trace!("starting sending external socket");
        futures::pin_mut!(to_socket);
        loop {
            if let Err(t) = self
                .process_to_websocket(to_websocket.next().await, &mut to_socket)
                .await
            {
                task = t;
            }
            // If during processing a message we encounter
            // a problem that can't be resolved then exit the loop.
            if let Task::Exit | Task::ExitNow = task {
                break;
            }
        }
        // Send close frame so the connection is
        // gracefully shutdown if we can.
        if let Task::Exit = task {
            to_socket
                .send(tungstenite::Message::Close(Some(CloseFrame {
                    code: CloseCode::Normal,
                    reason: "Shutting down sender".into(),
                })))
                .await
                // If we fail to send there's not much we can do.
                // Logging this will just create noise on shutdown.
                .ok();
        }
        self.0.shutdown();
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
        // Note that this task awaits on the outgoing messages
        // application stream and will close when that stream is closed.
        match msg {
            Some(msg) => {
                tracing::trace!(sending_msg = ?msg);

                // Map outgoing messages to wire messages.
                let msg = match msg {
                    OutgoingMessage::Close => return Task::exit(),
                    OutgoingMessage::Signal(msg) => WireMessage::Signal {
                        data: UnsafeBytes::from(msg).into(),
                    },
                    OutgoingMessage::Request(msg, register_response, tx_stale_response) => {
                        self.handle_outgoing_request(msg, register_response, tx_stale_response)
                            .await?
                    }
                    OutgoingMessage::Response(msg, id) => WireMessage::Response {
                        id,
                        data: msg.map(|m| UnsafeBytes::from(m).into()),
                    },
                    OutgoingMessage::StaleRequest(id) => {
                        return self.handle_stale_request(id).await;
                    }
                    OutgoingMessage::Pong(data) => {
                        // No need to deserialize, just send the data back
                        // and continue.
                        to_socket.send(tungstenite::Message::Pong(data)).await.ok();
                        return Task::cont();
                    }
                    OutgoingMessage::Debug(tx_requests_debug) => {
                        return self.handle_requests_debug(tx_requests_debug).await;
                    }
                };
                let msg = Self::serialize_msg(msg)?;

                // Write to_socket
                match to_socket.send(msg).await {
                    // Successful send.
                    Ok(_) => Task::cont(),
                    // Connection is already closed so exit immediately.
                    Err(tungstenite::Error::ConnectionClosed) => Task::exit_now(),
                    Err(e) => {
                        // If write fails then close both connections gracefully.
                        tracing::error!(to_socket_error = ?e);
                        Task::exit()
                    }
                }
            }
            // Stream from the application has closed.
            None => Task::exit(),
        }
    }

    #[instrument(skip(
        self,
        from_socket,
        from_websocket,
        send_response,
        shutdown_to_socket_immediately
    ))]
    /// Task that takes in messages from the network.
    async fn run_from_socket(
        self,
        from_socket: impl futures::stream::Stream<
            Item = std::result::Result<tungstenite::Message, tungstenite::Error>,
        >,
        mut from_websocket: TxFromWebsocket,
        mut send_response: TxToWebsocket,
        shutdown_to_socket_immediately: Trigger,
    ) {
        let mut task = Task::Continue;
        tracing::trace!("starting receiving from external socket");
        futures::pin_mut!(from_socket);

        // Note that this task awaits on the incoming external socket stream
        // and will close when that connection closes.
        loop {
            let msg = from_socket.next().await;
            if let Err(t) = self
                .process_from_websocket(msg, &mut from_websocket, &mut send_response)
                .await
            {
                task = t;
            }
            // If during processing a message we encounter
            // a problem that can't be resolved then exit the loop.
            if let Task::Exit | Task::ExitNow = task {
                break;
            }
        }
        // Try a graceful shutdown.
        if let Task::Exit = task {
            // If we can successfully send a close message
            // to the "to socket" task then we don't need to
            // force it to shutdown immediately.
            if send_response
                .send_timeout(OutgoingMessage::Close, CLOSE_TIMEOUT)
                .await
                .is_ok()
            {
                // Stops this from canceling the to socket
                // stream on drop.
                shutdown_to_socket_immediately.disable();
            }
        }
        self.0.shutdown();
        tracing::trace!("exiting receiving from external socket");
    }

    /// Process messages coming from the network and forward
    /// them onto the application.
    async fn process_from_websocket(
        &self,
        msg: Option<std::result::Result<tungstenite::Message, tungstenite::Error>>,
        from_websocket: &mut TxFromWebsocket,
        send_response: &mut TxToWebsocket,
    ) -> Loop<()> {
        match msg {
            Some(Ok(msg)) => {
                tracing::trace!(received_msg = ?msg);

                // Deserialize the incoming wire message.
                match msg {
                    tungstenite::Message::Binary(bytes) => {
                        let msg = Self::deserialize_message(bytes)?;
                        let (msg, resp) = match msg {
                            WireMessage::Signal { data } => {
                                (Self::deserialize_bytes(data)?, Respond::Signal)
                            }
                            WireMessage::Request { data, id } => Self::handle_incoming_request(
                                send_response,
                                Self::deserialize_bytes(data)?,
                                id,
                            ),
                            WireMessage::Response {
                                data: Some(data),
                                id,
                            } => {
                                // Send this response to the WebsocketSender who
                                // made the original request.
                                return self
                                    .handle_incoming_response(
                                        Some(Self::deserialize_bytes(data)?),
                                        id,
                                    )
                                    .await;
                            }
                            WireMessage::Response { data: None, id } => {
                                tracing::trace!(canceled = ?id);
                                // A response that has been canceled.
                                // This means the other sides receiver has shutdown.
                                return self.handle_incoming_response(None, id).await;
                            }
                        };

                        // Forward the incoming message to the WebsocketReceiver.
                        if from_websocket
                            .send(IncomingMessage::Msg(msg, resp))
                            .await
                            .is_err()
                        {
                            // We received a message for the receiver but the
                            // receiver has been dropped so we need to shutdown this
                            // connection because the other side is expecting there to
                            // be a receiver.
                            // Note this will not happen if we are only receiving responses.
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
                            // We successfully sent the close to the receiver now we
                            // wait for acknowledgement or timeout.
                            tokio::time::timeout(CLOSE_TIMEOUT, resp).await.ok();
                        }
                        Task::exit_now()
                    }
                    tungstenite::Message::Ping(data) => {
                        // Received a ping, immediately respond with a pong.
                        send_response.send(OutgoingMessage::Pong(data)).await.ok();
                        Task::cont()
                    }
                    m => {
                        // Received a text message which we don't support.
                        tracing::error!("Websocket: Bad message type {:?}", m);
                        Task::cont()
                    }
                }
            }
            Some(Err(e)) => {
                // We got an error from the connection so we should
                // exit immediately.

                // TODO: Check if some of these errors are recoverable.
                tracing::error!(websocket_error_from_network = ?e);
                Task::exit_now()
            }
            // Incoming network stream has closed.
            // Try closing the outgoing stream incase it
            // hasn't already closed.
            None => Task::exit(),
        }
    }

    /// Handling a request coming in from the network
    /// and reply with a response.
    fn handle_incoming_request(
        send_response: &mut TxToWebsocket,
        msg: SerializedBytes,
        id: u64,
    ) -> (SerializedBytes, Respond) {
        let resp = {
            // Get the sender to the "to socket" task so we can reply.
            let mut send_response = send_response.clone();
            // If the reply closure is never run and only dropped we want
            // to send a canceled response to the other sides WebsocketSender.
            let cancel_response = CancelResponse::new(send_response.clone(), id);

            // Callback to respond to the request
            move |msg| {
                async move {
                    let msg = OutgoingMessage::Response(Some(msg), id);

                    // Send the response to the to_socket task
                    send_response
                        .send(msg)
                        .await
                        .map_err(|_| WebsocketError::FailedToSendResp)?;
                    // Response sent, don't send cancel.
                    cancel_response.response_sent();
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

    /// Handle a request going out to the network.
    async fn handle_outgoing_request(
        &self,
        msg: SerializedBytes,
        register_response: RegisterResponse,
        tx_stale_request: TxStaleRequest,
    ) -> Loop<WireMessage> {
        // If the actor has closed we can't register this response.
        if !self.0.is_active() {
            tracing::error!("Actor is closed");
            return Task::exit();
        }
        // Register outgoing message with the actor.
        let id = match self
            .0
            .invoke(move |state| GhostResult::Ok(state.responses.register(register_response)))
            .await
        {
            Ok(id) => id,
            Err(e) => {
                // Failed to register so something is
                // wrong with the actor and we should shutdown.
                tracing::error!(?e);
                return Task::exit();
            }
        };
        // Send the id back to create the stale request guard.
        if let Err(id) = tx_stale_request.send(id) {
            // If we fail to send the id that means the requester
            // has dropped so we should clean up the stale request.
            match self.handle_stale_request(id).await {
                Ok(_) => unreachable!("handle_stale_request always continues or exits the loop"),
                Err(task) => return Err(task),
            }
        }
        let data = UnsafeBytes::from(msg).into();
        Ok(WireMessage::Request { data, id })
    }

    /// Handle a request that has gone stale.
    async fn handle_stale_request(&self, id: u64) -> Loop<()> {
        // If the actor has closed we can't clean up this response.
        if !self.0.is_active() {
            tracing::error!("Actor is closed");
            return Task::exit();
        }
        tracing::trace!(here = line!());
        match self
            .0
            .invoke(move |state| GhostResult::Ok(state.responses.pop(id)))
            .await
        {
            Ok(_) => Task::cont(),
            Err(e) => {
                // Failed to clean up request so something is
                // wrong with the actor and we should shutdown.
                tracing::error!(?e);
                Task::exit()
            }
        }
    }

    /// Get the current state of the requests for debugging.
    async fn handle_requests_debug(&self, tx_requests_debug: TxRequestsDebug) -> Loop<()> {
        // If the actor has closed we can't clean up this response.
        if !self.0.is_active() {
            tracing::error!("Actor is closed");
            return Task::exit();
        }
        match self
            .0
            .invoke(move |state| GhostResult::Ok(state.responses.debug()))
            .await
        {
            Ok(state) => {
                tx_requests_debug.send(state).ok();
                Task::cont()
            }
            Err(e) => {
                // Failed to get debug state, something is
                // wrong with the actor and we should shutdown.
                tracing::error!(?e);
                Task::exit()
            }
        }
    }

    /// Handle a response coming in from the network.
    async fn handle_incoming_response(&self, msg: Option<SerializedBytes>, id: u64) -> Loop<()> {
        // If the actor has closed we can't find the registered response.
        if !self.0.is_active() {
            tracing::error!("Actor is closed");
            return Task::exit();
        }
        // Find the registered response and respond.
        let r = self
            .0
            .invoke(move |state| GhostResult::Ok(state.responses.pop(id)))
            .await
            .map_err(WebsocketError::from)
            .and_then(|response| match response {
                Some(r) => r.respond(msg),
                None => {
                    // We don't want to error here because a bad response
                    // shouldn't shutdown the connection.
                    tracing::warn!("Websocket: Received response for request that doesn't exist or has gone stale");
                    Ok(())
                }
            });

        match r {
            Ok(_) => {
                // We are done responding, nothing
                // else to do in this loop so continue.
                return Task::cont();
            }
            Err(e) => {
                // Failed to handle the response so we need to
                // shutdown.
                tracing::error!(handle_response_error = ?e);
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
    /// Try to deserialize the data and continue to next
    /// message if failure.
    fn deserialize_bytes(data: Vec<u8>) -> Loop<SerializedBytes> {
        let msg: Result<SerializedBytes, _> = UnsafeBytes::from(data).try_into();
        match msg {
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
    fn new() -> Self {
        Self {
            responses: HashMap::new(),
            index: 0,
        }
    }

    /// Register an outgoing request with it's response.
    fn register(&mut self, response: RegisterResponse) -> u64 {
        // Get the index for this response and increment for the next response.
        let index = self.index;
        self.index += 1;

        self.responses.insert(index, response);
        index
    }

    /// Retrieve the response at this id.
    fn pop(&mut self, id: u64) -> Option<RegisterResponse> {
        self.responses.remove(&id)
    }

    /// Show outstanding responses.
    fn debug(&self) -> (Vec<u64>, u64) {
        (self.responses.keys().copied().collect(), self.index)
    }
}

impl Drop for PairShutdown {
    fn drop(&mut self) {
        // Try to send a close to the "to socket task".
        // This is optimistic because the task may already be
        // shutting down.
        self.close_to_socket.try_send(OutgoingMessage::Close).ok();
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::connect;
    use crate::WebsocketListener;
    use url2::url2;

    #[tokio::test(threaded_scheduler)]
    async fn test_register_response() {
        observability::test_run().ok();
        let (handle, mut listener) = WebsocketListener::bind_with_handle(
            url2!("ws://127.0.0.1:0"),
            Arc::new(WebsocketConfig::default()),
        )
        .await
        .unwrap();
        let binding = handle.local_addr().clone();
        let sjh = tokio::task::spawn(async move {
            let (_, _receiver) = listener
                .next()
                .instrument(tracing::debug_span!("next_server_connection"))
                .await
                .unwrap()
                .unwrap();

            listener
                .next()
                .instrument(tracing::debug_span!("next_server_connection"))
                .await;
        });
        let (mut sender, _) = connect(binding.clone(), Arc::new(WebsocketConfig::default()))
            .instrument(tracing::debug_span!("client"))
            .await
            .unwrap();

        let msg = SerializedBytes::from(UnsafeBytes::from(vec![0u8]));
        sender
            .request_timeout::<_, SerializedBytes, _, _>(msg, std::time::Duration::from_secs(1))
            .await
            .ok();
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
        let state = sender.debug().await.unwrap();
        assert_eq!(state, (vec![], 1));

        // - Connect and drop to close the server.
        connect(binding, Arc::new(WebsocketConfig::default()))
            .instrument(tracing::debug_span!("client"))
            .await
            .unwrap();
        sjh.await.unwrap();
    }
}
