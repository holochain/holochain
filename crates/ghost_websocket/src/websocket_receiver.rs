use std::sync::Arc;

use holochain_serialized_bytes::SerializedBytes;
use must_future::MustBoxFuture;
use stream_cancel::Trigger;
use stream_cancel::Valved;

use crate::websocket::PairShutdown;
use crate::websocket::RxFromWebsocket;
use crate::WebsocketResult;

pub struct WebsocketReceiver {
    rx_from_websocket: Valved<Valved<RxFromWebsocket>>,
    handle: Option<ReceiverHandle>,
    __pair_shutdown: Arc<PairShutdown>,
}

pub type Response = Box<
    dyn FnOnce(SerializedBytes) -> MustBoxFuture<'static, WebsocketResult<()>>
        + 'static
        + Send
        + Sync,
>;

pub enum Respond {
    Signal,
    Request(Response),
}

/// Shuts down the receiver on drop or
/// if you call close.
pub struct ReceiverHandle {
    shutdown: Trigger,
}

pub(crate) enum IncomingMessage {
    Close {
        acknowledge: tokio::sync::oneshot::Sender<()>,
    },
    Msg(SerializedBytes, Respond),
}

pub type WebsocketMessage = (SerializedBytes, Respond);

impl WebsocketReceiver {
    pub(crate) fn new(
        rx_from_websocket: Valved<RxFromWebsocket>,
        pair_shutdown: Arc<PairShutdown>,
    ) -> Self {
        let (shutdown, rx_from_websocket) = Valved::new(rx_from_websocket);
        let handle = Some(ReceiverHandle { shutdown });
        Self {
            rx_from_websocket,
            handle,
            __pair_shutdown: pair_shutdown,
        }
    }
    pub fn take_handle(&mut self) -> Option<ReceiverHandle> {
        self.handle.take()
    }
}

impl futures::stream::Stream for WebsocketReceiver {
    type Item = WebsocketMessage;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::task::Poll::*;
        let p = std::pin::Pin::new(&mut self.rx_from_websocket);
        match futures::stream::Stream::poll_next(p, cx) {
            Ready(Some(IncomingMessage::Msg(msg, resp))) => Ready(Some((msg, resp))),
            Ready(Some(IncomingMessage::Close { acknowledge })) => {
                acknowledge.send(()).ok();
                Ready(None)
            }
            Ready(None) => Ready(None),
            Pending => Pending,
        }
    }
}

impl ReceiverHandle {
    /// Shutdown the receiver stream.
    pub fn close(self) {
        self.shutdown.cancel()
    }

    /// Close the receiver when the future resolves to true.
    /// If the future returns false the receiver will not be closed.
    pub async fn close_on<F>(self, f: F)
    where
        F: std::future::Future<Output = bool>,
    {
        if f.await {
            self.close()
        }
    }
}

impl Respond {
    pub fn is_request(&self) -> bool {
        match self {
            Respond::Signal => false,
            Respond::Request(_) => true,
        }
    }
    /// Try to respond. If this is a signal then
    /// the call is a noop.
    pub async fn respond(self, msg: SerializedBytes) -> WebsocketResult<()> {
        match self {
            Respond::Signal => Ok(()),
            Respond::Request(r) => r(msg).await,
        }
    }
}
