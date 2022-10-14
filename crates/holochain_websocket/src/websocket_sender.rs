use futures::FutureExt;
use futures::StreamExt;
use holochain_serialized_bytes::{SerializedBytes, SerializedBytesError};
use serde::{de::DeserializeOwned, Serialize};
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

#[derive(Debug)]
/// If when the response future is finished (or dropped) the response
/// hasn't arrived then on drop this will remove the stale request.
pub(crate) struct StaleRequest(bool, TxToWebsocket, u64);
pub(crate) type TxStaleRequest = tokio::sync::oneshot::Sender<u64>;

/// Get the current state of the registered responses.
pub(crate) type TxRequestsDebug = tokio::sync::oneshot::Sender<(Vec<u64>, u64)>;

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
    Request(SerializedBytes, RegisterResponse, TxStaleRequest),
    Response(Option<SerializedBytes>, u64),
    StaleRequest(u64),
    Pong(Vec<u8>),
    #[allow(dead_code)]
    Debug(TxRequestsDebug),
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
    pub async fn request_timeout<I, O>(
        &self,
        msg: I,
        timeout: std::time::Duration,
    ) -> WebsocketResult<O>
    where
        I: std::fmt::Debug,
        O: std::fmt::Debug,
        WebsocketError: From<SerializedBytesError>,
        I: Serialize,
        O: DeserializeOwned,
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
    pub async fn request<I, O>(&self, msg: I) -> WebsocketResult<O>
    where
        I: std::fmt::Debug,
        O: std::fmt::Debug,
        WebsocketError: From<SerializedBytesError>,
        I: Serialize,
        O: DeserializeOwned,
    {
        use holochain_serialized_bytes as hsb;
        tracing::trace!("Sending");

        let (tx_resp, rx_resp) = tokio::sync::oneshot::channel();
        let (tx_stale_resp, rx_stale_resp) = tokio::sync::oneshot::channel();
        let mut rx_resp = self.listener_shutdown.wrap(rx_resp.into_stream());
        let resp = RegisterResponse::new(tx_resp);
        let msg = OutgoingMessage::Request(
            hsb::UnsafeBytes::from(hsb::encode(&msg)?).try_into()?,
            resp,
            tx_stale_resp,
        );

        self.tx_to_websocket
            .send(msg)
            .await
            .map_err(|_| WebsocketError::Shutdown)?;

        tracing::trace!("Sent");
        let id = rx_stale_resp.await.map_err(|_| WebsocketError::Shutdown)?;
        let stale_request_guard = StaleRequest::new(self.tx_to_websocket.clone(), id);

        let sb: SerializedBytes = rx_resp
            .next()
            .await
            .ok_or(WebsocketError::Shutdown)?
            .map_err(|_| WebsocketError::FailedToRecvResp)?
            .ok_or(WebsocketError::FailedToRecvResp)?;
        let resp: O = hsb::decode(&Vec::from(hsb::UnsafeBytes::from(sb)))?;
        stale_request_guard.response_received();
        Ok(resp)
    }

    #[tracing::instrument(skip(self))]
    /// Send a message to the other side that doesn't require a response.
    /// There is no guarantee this message will arrive. If you need confirmation
    /// of receipt use [`WebsocketSender::request`].
    pub async fn signal<I, E>(&self, msg: I) -> WebsocketResult<()>
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

    #[cfg(test)]
    pub(crate) async fn debug(&self) -> WebsocketResult<(Vec<u64>, u64)> {
        let (tx_resp, rx_resp) = tokio::sync::oneshot::channel();
        let msg = OutgoingMessage::Debug(tx_resp);
        self.tx_to_websocket
            .send(msg)
            .await
            .map_err(|_| WebsocketError::Shutdown)?;
        Ok(rx_resp.await.map_err(|_| WebsocketError::Shutdown)?)
    }
}

impl StaleRequest {
    /// To remove responses we need the channel to the websocket
    /// and the id of the request.
    pub fn new(send_response: TxToWebsocket, id: u64) -> Self {
        Self(true, send_response, id)
    }
    /// The response has been received so don't cancel on drop.
    pub fn response_received(mut self) {
        self.0 = false;
    }
}

impl Drop for StaleRequest {
    fn drop(&mut self) {
        // If this response hasn't been received then remove the registered response.
        if self.0 {
            let tx = self.1.clone();
            let id = self.2;
            tokio::spawn(async move {
                if let Err(e) = tx.send(OutgoingMessage::StaleRequest(id)).await {
                    tracing::warn!("Failed to remove stale response on drop {:?}", e);
                }
                tracing::trace!("Removed stale response on drop");
            });
        }
    }
}
