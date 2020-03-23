//! defines the write/send half of a websocket pair

use crate::*;

/// The sender half allows making outgoing requests to the websocket
/// This struct is cheaply clone-able.
#[derive(Clone)]
pub struct WebsocketSender {
    sender: RawSender,
}

impl WebsocketSender {
    /// internal constructor
    pub(crate) fn priv_new(sender: RawSender) -> Self {
        Self { sender }
    }

    /// Send a message to the remote end
    pub async fn send(&mut self, msg: tungstenite::Message) -> Result<()> {
        self.sender
            .send(msg)
            .await
            .map_err(|e| Error::new(ErrorKind::Other, e))
    }
}
