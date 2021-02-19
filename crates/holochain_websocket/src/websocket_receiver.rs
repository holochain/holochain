use std::sync::Arc;

use holochain_serialized_bytes::SerializedBytes;
use must_future::MustBoxFuture;
use stream_cancel::Trigger;
use stream_cancel::Valved;
use url2::Url2;

use crate::websocket::PairShutdown;
use crate::websocket::RxFromWebsocket;
use crate::websocket::TxToWebsocket;
use crate::OutgoingMessage;
use crate::WebsocketResult;

/// Receive signals and requests from a connection.
///
/// # Example
/// ```no_run
/// use futures::stream::StreamExt;
/// use holochain_websocket::*;
/// use url2::url2;
///
/// #[tokio::main]
/// async fn main() {
///     let (_, mut recv) = connect(
///         url2!("ws://127.0.0.1:12345"),
///         std::sync::Arc::new(WebsocketConfig::default()),
///     )
///     .await
///     .unwrap();
///     while let Some((msg, resp)) = recv.next().await {
///         if resp.is_request() {
///             resp.respond(msg).await.unwrap();
///         }
///     }
/// }
/// ```
pub struct WebsocketReceiver {
    rx_from_websocket: Valved<Valved<RxFromWebsocket>>,
    remote_addr: Url2,
    handle: Option<ReceiverHandle>,
    __pair_shutdown: Arc<PairShutdown>,
}

/// Closure for responding to a [`Respond::Request`]
pub type Response = Box<
    dyn FnOnce(SerializedBytes) -> MustBoxFuture<'static, WebsocketResult<()>>
        + 'static
        + Send
        + Sync,
>;

/// The response half to a [`WebsocketMessage`].
/// If this message is a request [`Respond::is_request`] you can
/// respond with [`Respond::respond`].
pub enum Respond {
    /// This message is a signal so there is nothing to respond to.
    Signal,
    /// Respond to an incoming request.
    Request(Response),
}

/// If a request is in the queue at shutdown this will send
/// a cancellation response on drop.
pub(crate) struct CancelResponse(bool, TxToWebsocket, u64);

/// Shuts down the receiver on drop or
/// if you call close.
pub struct ReceiverHandle {
    shutdown: Trigger,
}

/// A message coming **in** from the external socket.
pub(crate) enum IncomingMessage {
    Close {
        acknowledge: tokio::sync::oneshot::Sender<()>,
    },
    Msg(SerializedBytes, Respond),
}

/// The [`SerializedBytes`] message contents and the [`Respond`] from the [`WebsocketReceiver`] [`Stream`](futures::Stream).
pub type WebsocketMessage = (SerializedBytes, Respond);

impl WebsocketReceiver {
    pub(crate) fn new(
        rx_from_websocket: Valved<RxFromWebsocket>,
        remote_addr: Url2,
        pair_shutdown: Arc<PairShutdown>,
    ) -> Self {
        let (shutdown, rx_from_websocket) = Valved::new(rx_from_websocket);
        let handle = Some(ReceiverHandle { shutdown });
        Self {
            rx_from_websocket,
            remote_addr,
            handle,
            __pair_shutdown: pair_shutdown,
        }
    }

    /// Take the [`ReceiverHandle`] from this receiver so you can shut down the stream.
    pub fn take_handle(&mut self) -> Option<ReceiverHandle> {
        self.handle.take()
    }
    /// get the remote url this websocket is connected to.
    pub fn remote_addr(&self) -> &Url2 {
        &self.remote_addr
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
        tracing::trace!("Closing Receiver");
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
    /// Check if this message is a request or a signal.
    pub fn is_request(&self) -> bool {
        match self {
            Respond::Signal => false,
            Respond::Request(_) => true,
        }
    }
    /// Respond to a request.
    /// If this is a signal then the call is a noop.
    pub async fn respond(self, msg: SerializedBytes) -> WebsocketResult<()> {
        match self {
            Respond::Signal => Ok(()),
            Respond::Request(r) => r(msg).await,
        }
    }
}

impl CancelResponse {
    /// To cancel the response we need the channel to the websocket
    /// and the id of the request.
    pub fn new(send_response: TxToWebsocket, id: u64) -> Self {
        Self(true, send_response, id)
    }
    /// The response has been sent so don't cancel on drop.
    pub fn response_sent(mut self) {
        self.0 = false;
    }
}

impl Drop for CancelResponse {
    fn drop(&mut self) {
        // If this response hasn't been sent then send a None response.
        if self.0 {
            let mut tx = self.1.clone();
            let id = self.2;
            tokio::spawn(async move {
                if let Err(e) = tx.send(OutgoingMessage::Response(None, id)).await {
                    tracing::warn!("Failed to cancel response on drop {:?}", e);
                }
            });
        }
    }
}
