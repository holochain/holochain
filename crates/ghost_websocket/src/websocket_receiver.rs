use std::io::Result;

use crate::websocket::RxFromWebsocket;
// WebsocketReceiver // Stream (RequestData, Response)
// RequestData // Bytes the actual message
// Response // fn once(Message)
pub struct WebsocketReceiver {
    rx_from_websocket: RxFromWebsocket,
}

// pub type Response = Box<dyn FnOnce(()) -> ()>;
pub type Response = ();

pub struct WebsocketReceiverHandle;
pub type Message = (String, Response);

impl WebsocketReceiver {
    pub(crate) fn new(rx_from_websocket: RxFromWebsocket) -> Self {
        Self { rx_from_websocket }
    }
    pub async fn connect(
        self,
    ) -> Result<(
        WebsocketReceiverHandle,
        impl futures::stream::Stream<Item = Message>,
    )> {
        let handle = WebsocketReceiverHandle {};
        let stream = self.rx_from_websocket;
        // let stream = futures::stream::pending::<Result<(String, Response)>>();
        Ok((handle, stream))
    }
}
