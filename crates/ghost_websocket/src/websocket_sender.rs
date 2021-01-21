use websocket::TxToWebsocket;

use crate::websocket;
use crate::websocket::Websocket;
use std::io::Result;

#[derive(Debug, Clone)]
pub struct WebsocketSender {
    actor: Websocket,
    tx_to_websocket: TxToWebsocket,
}

impl WebsocketSender {
    pub(crate) fn new(actor: Websocket, tx_to_websocket: TxToWebsocket) -> Self {
        Self {
            actor,
            tx_to_websocket,
        }
    }

    pub async fn request(&self, _msg: ()) -> Result<()> {
        todo!()
    }

    pub async fn signal(&self, msg: &str) -> Result<()> {
        tracing::debug!(sending = ?msg);
        let mut sender = self.tx_to_websocket.clone();
        if let Err(_) = sender.send(msg.into()).await {
            tracing::error!("Websocket receiver dropped");
        }
        tracing::debug!(sent = ?msg);
        Ok(())
    }
}
