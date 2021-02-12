use futures::FutureExt;
use futures::StreamExt;
use holochain_serialized_bytes::SerializedBytes;
use stream_cancel::Valve;
use websocket::PairShutdown;
use websocket::TxToWebsocket;

use crate::websocket;
use crate::WebsocketError;
use crate::WebsocketResult;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::sync::Arc;

#[derive(Debug, Clone)]
/// The sender half of an active connection.
///
/// # Example
/// ```no_run
/// use holochain_serialized_bytes::prelude::*;
/// use holochain_websocket::*;
/// use std::time::Duration;
/// use url2::url2;
///
/// #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
/// struct TestMessage(pub String);
///
/// #[tokio::main]
/// async fn main() {
///     let (mut send, _) = connect(
///         url2!("ws://127.0.0.1:12345"),
///         std::sync::Arc::new(WebsocketConfig::default()),
///     )
///     .await
///     .unwrap();
///
///     send.signal(TestMessage("Hey".to_string())).await.unwrap();
///     let resp: TestMessage = send.request(TestMessage("Hey".to_string())).await.unwrap();
///     println!("Got {:?}", resp);
///
///     match send
///         .request_timeout(TestMessage("Hey".to_string()), Duration::from_secs(1))
///         .await
///     {
///         Ok(r) => {
///             let resp: TestMessage = r;
///             println!("Got {:?}", resp);
///         }
///         Err(WebsocketError::RespTimeout) => eprintln!("Failed to get a response in 1 second"),
///         Err(e) => eprintln!("Got an error sending a request {:?}", e),
///     }
/// }
/// ```
pub struct WebsocketSender {
    tx_to_websocket: TxToWebsocket,
    listener_shutdown: Valve,
    __pair_shutdown: Arc<PairShutdown>,
}

#[derive(Debug)]
/// Register a response for an outgoing request.
pub(crate) struct RegisterResponse {
    respond: tokio::sync::oneshot::Sender<Option<SerializedBytes>>,
}

impl RegisterResponse {
    pub(crate) fn new(respond: tokio::sync::oneshot::Sender<Option<SerializedBytes>>) -> Self {
        Self { respond }
    }

    /// The request has comeback from the other side so we can respond to
    /// the awaiting future here.
    pub(crate) fn respond(self, msg: Option<SerializedBytes>) -> WebsocketResult<()> {
        tracing::trace!(sending_resp = ?msg);
        self.respond
            .send(msg)
            .map_err(|_| WebsocketError::FailedToSendResp)
    }
}

#[derive(Debug)]
/// A message going **out** to the external socket.
pub(crate) enum OutgoingMessage {
    Close,
    Signal(SerializedBytes),
    Request(SerializedBytes, RegisterResponse),
    Response(Option<SerializedBytes>, u32),
    Pong(Vec<u8>),
}

impl WebsocketSender {
    pub(crate) fn new(
        tx_to_websocket: TxToWebsocket,
        listener_shutdown: Valve,
        pair_shutdown: Arc<PairShutdown>,
    ) -> Self {
        Self {
            tx_to_websocket,
            listener_shutdown,
            __pair_shutdown: pair_shutdown,
        }
    }

    #[tracing::instrument(skip(self))]
    /// Make a request to for the other side to respond to.
    ///
    /// Note:
    /// There is no timeouts in this code. You either need to wrap
    /// this future in a timeout or use [`WebsocketSender::request_timeout`].
    pub async fn request_timeout<I, O, E, E2>(
        &mut self,
        msg: I,
        timeout: std::time::Duration,
    ) -> WebsocketResult<O>
    where
        I: std::fmt::Debug,
        O: std::fmt::Debug,
        WebsocketError: From<E>,
        WebsocketError: From<E2>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E2>,
    {
        match tokio::time::timeout(timeout, self.request(msg)).await {
            Ok(r) => r,
            Err(_) => Err(WebsocketError::RespTimeout),
        }
    }

    #[tracing::instrument(skip(self))]
    /// Make a request to for the other side to respond to.
    ///
    /// Note:
    /// There is no timeouts in this code. You either need to wrap
    /// this future in a timeout or use [`WebsocketSender::request_timeout`].
    pub async fn request<I, O, E, E2>(&mut self, msg: I) -> WebsocketResult<O>
    where
        I: std::fmt::Debug,
        O: std::fmt::Debug,
        WebsocketError: From<E>,
        WebsocketError: From<E2>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E2>,
    {
        tracing::trace!("Sending");
        let (tx_resp, rx_resp) = tokio::sync::oneshot::channel();
        let mut rx_resp = self.listener_shutdown.wrap(rx_resp.into_stream());
        let resp = RegisterResponse::new(tx_resp);
        let msg = OutgoingMessage::Request(msg.try_into()?, resp);

        self.tx_to_websocket
            .send(msg)
            .await
            .map_err(|_| WebsocketError::Shutdown)?;

        tracing::trace!("Sent");

        Ok(rx_resp
            .next()
            .await
            .ok_or(WebsocketError::Shutdown)?
            .map_err(|_| WebsocketError::FailedToRecvResp)?
            .ok_or(WebsocketError::FailedToRecvResp)?
            .try_into()?)
    }

    #[tracing::instrument(skip(self))]
    /// Send a message to the other side that doesn't require a response.
    /// There is no guarantee this message will arrive. If you need confirmation
    /// of receipt use [`WebsocketSender::request`].
    pub async fn signal<I, E>(&mut self, msg: I) -> WebsocketResult<()>
    where
        I: std::fmt::Debug,
        WebsocketError: From<E>,
        SerializedBytes: TryFrom<I, Error = E>,
    {
        tracing::trace!("Sending");
        let msg = OutgoingMessage::Signal(msg.try_into()?);

        self.tx_to_websocket
            .send(msg)
            .await
            .map_err(|_| WebsocketError::Shutdown)?;

        tracing::trace!("Sent");
        Ok(())
    }
}
