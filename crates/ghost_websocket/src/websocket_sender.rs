use holochain_serialized_bytes::SerializedBytes;
use websocket::TxToWebsocket;

use crate::websocket;
use crate::WebsocketError;
use crate::WebsocketResult;
use std::convert::TryFrom;
use std::convert::TryInto;

#[derive(Debug, Clone)]
pub struct WebsocketSender {
    tx_to_websocket: TxToWebsocket,
}

#[derive(Debug)]
pub(crate) struct RegisterResponse {
    respond: tokio::sync::oneshot::Sender<SerializedBytes>,
}

impl RegisterResponse {
    pub(crate) fn respond(self, msg: SerializedBytes) -> WebsocketResult<()> {
        self.respond
            .send(msg)
            .map_err(|_| WebsocketError::FailedToSendResp)
    }
}

#[derive(Debug)]
pub(crate) enum OutgoingMessage {
    Signal(SerializedBytes),
    Request(SerializedBytes, RegisterResponse),
    Response(SerializedBytes, u32),
}

impl WebsocketSender {
    pub(crate) fn new(tx_to_websocket: TxToWebsocket) -> Self {
        Self { tx_to_websocket }
    }

    #[tracing::instrument(skip(self))]
    pub async fn request<I, O, E, E2>(&self, msg: I) -> WebsocketResult<O>
    where
        I: std::fmt::Debug,
        O: std::fmt::Debug,
        WebsocketError: From<E>,
        WebsocketError: From<E2>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E2>,
    {
        tracing::debug!("Sending");
        let mut sender = self.tx_to_websocket.clone();
        let (tx_resp, rx_resp) = tokio::sync::oneshot::channel();
        let resp = RegisterResponse { respond: tx_resp };
        let msg = OutgoingMessage::Request(msg.try_into()?, resp);
        if let Err(_) = sender.send(msg).await {
            tracing::error!("Websocket receiver dropped");
        }
        tracing::debug!("Sent");
        Ok(rx_resp
            .await
            .map_err(|_| WebsocketError::FailedToRecvResp)?
            .try_into()?)
    }

    #[tracing::instrument(skip(self))]
    pub async fn signal<I, E>(&self, msg: I) -> WebsocketResult<()>
    where
        I: std::fmt::Debug,
        WebsocketError: From<E>,
        SerializedBytes: TryFrom<I, Error = E>,
    {
        tracing::debug!("Sending");
        let mut sender = self.tx_to_websocket.clone();
        let msg = OutgoingMessage::Signal(msg.try_into()?);
        if let Err(_) = sender.send(msg).await {
            tracing::error!("Websocket receiver dropped");
        }
        tracing::debug!("Sent");
        Ok(())
    }
}
