use std::sync::Arc;

use futures::SinkExt;
use futures::StreamExt;
use stream_cancel::Trigger;
use stream_cancel::Valved;
use tracing::instrument;
use tracing::Instrument;

use ghost_actor::*;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::CloseFrame;

use crate::util::ToFromSocket;
use crate::Message;
use crate::WebsocketConfig;
use crate::WebsocketReceiver;
use crate::WebsocketSender;

// This task creates WebsocketSender Actor and
// Websocket Actor which is responsible for message id hash map
// which tracks requests to responses.
#[derive(Debug, Clone)]
pub struct Websocket(GhostActor<WebsocketInner>);

struct WebsocketInner {}

// Channel to the websocket
pub type TxToWebsocket = tokio::sync::mpsc::Sender<String>;
type RxToWebsocket = tokio::sync::mpsc::Receiver<String>;

// Channel from the websocket
type TxFromWebsocket = tokio::sync::mpsc::Sender<Message>;
pub type RxFromWebsocket = tokio::sync::mpsc::Receiver<Message>;

impl Websocket {
    #[instrument(skip(config, socket))]
    pub fn create_ends(
        config: Arc<WebsocketConfig>,
        socket: ToFromSocket,
    ) -> (WebsocketSender, WebsocketReceiver) {
        let (tx_to_websocket, rx_to_websocket) = tokio::sync::mpsc::channel(config.max_send_queue);
        let (tx_from_websocket, rx_from_websocket) =
            tokio::sync::mpsc::channel(config.max_send_queue);
        let actor = Websocket::new(config, socket, rx_to_websocket, tx_from_websocket);
        let sender = WebsocketSender::new(actor, tx_to_websocket);
        let receiver = WebsocketReceiver::new(rx_from_websocket);
        (sender, receiver)
    }

    #[instrument(skip(config, socket, rx_to_websocket, tx_from_websocket))]
    pub fn new(
        config: Arc<WebsocketConfig>,
        socket: ToFromSocket,
        rx_to_websocket: RxToWebsocket,
        tx_from_websocket: TxFromWebsocket,
    ) -> Self {
        let (actor, driver) = GhostActor::new(WebsocketInner {});
        tokio::task::spawn(driver);
        Self::run_socket(actor.clone(), socket, rx_to_websocket, tx_from_websocket);

        Self(actor)
    }

    fn run_socket(
        actor: GhostActor<WebsocketInner>,
        socket: ToFromSocket,
        inbound: RxToWebsocket,
        outbound: TxFromWebsocket,
    ) {
        let (to_socket, from_socket) = socket.split();
        // let shutdown = Arc::new(shutdown);
        let (shutdown_from_socket, from_socket) = Valved::new(from_socket);
        let (shutdown_to_socket, inbound) = Valved::new(inbound);
        // let from_socket = from_socket.map_err(|e| Error::new(ErrorKind::Other, e));
        tokio::task::spawn(
            Self::run_to_socket(to_socket, inbound, shutdown_from_socket).in_current_span(),
        );
        tokio::task::spawn(
            Self::run_from_socket(from_socket, outbound, shutdown_to_socket).in_current_span(),
        );
    }

    #[instrument(skip(to_socket, inbound, _shutdown_from_socket))]
    async fn run_to_socket(
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
                    // Write to_socket
                    if let Err(e) = to_socket.send(msg.into()).await {
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
        tracing::trace!("exiting sending to external socket");
    }

    #[instrument(skip(from_socket, outbound, _shutdown_to_socket))]
    async fn run_from_socket(
        from_socket: impl futures::stream::Stream<
            Item = std::result::Result<tungstenite::Message, tungstenite::error::Error>,
        >,
        mut outbound: TxFromWebsocket,
        _shutdown_to_socket: Trigger,
    ) {
        tracing::trace!("starting receiving from external socket");
        futures::pin_mut!(from_socket);
        'recv_loop: loop {
            match from_socket.next().await {
                Some(Ok(msg)) => {
                    tracing::debug!(?msg);
                    match msg {
                        tungstenite::Message::Text(s) => {
                            if let Err(_) = outbound.send((s, ())).await {
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
