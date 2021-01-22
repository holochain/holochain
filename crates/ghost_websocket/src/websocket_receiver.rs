use holochain_serialized_bytes::SerializedBytes;
use must_future::MustBoxFuture;
use std::io::Result;

use crate::websocket::RxFromWebsocket;
use crate::WebsocketResult;

pub struct WebsocketReceiver {
    rx_from_websocket: RxFromWebsocket,
}

pub type Response = Box<
    dyn FnOnce(SerializedBytes) -> MustBoxFuture<'static, WebsocketResult<()>>
        + 'static
        + Send
        + Sync,
>;

pub struct WebsocketReceiverHandle;
pub type IncomingMessage = (SerializedBytes, Response);

impl WebsocketReceiver {
    pub(crate) fn new(rx_from_websocket: RxFromWebsocket) -> Self {
        Self { rx_from_websocket }
    }
    pub async fn connect(
        self,
    ) -> Result<(
        WebsocketReceiverHandle,
        impl futures::stream::Stream<Item = IncomingMessage>,
    )> {
        let handle = WebsocketReceiverHandle {};
        let stream = self.rx_from_websocket;
        Ok((handle, stream))
    }
}
