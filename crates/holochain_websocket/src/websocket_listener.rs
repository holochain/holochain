use futures::stream::BoxStream;
use futures::StreamExt;
use futures::TryStreamExt;
use std::io::Error;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use stream_cancel::Trigger;
use stream_cancel::Valve;
use tracing::instrument;

use url2::Url2;

use crate::util::addr_to_url;
use crate::util::url_to_addr;
use crate::websocket::Websocket;
use crate::WebsocketConfig;
use crate::WebsocketError;
use crate::WebsocketReceiver;
use crate::WebsocketResult;
use crate::WebsocketSender;

/// Listens for connecting clients.
///
/// # Example
/// ```no_run
/// use futures::stream::StreamExt;
/// use holochain_websocket::*;
/// use url2::url2;
///
/// #[tokio::main]
/// async fn main() {
///     let mut listener = WebsocketListener::bind(
///         url2!("ws://127.0.0.1:12345"),
///         std::sync::Arc::new(WebsocketConfig::default()),
///     )
///     .await
///     .unwrap();
///
///     while let Some(Ok((_send, _recv))) = listener.next().await {
///         // New connection
///     }
/// }
///```
pub struct WebsocketListener {
    handle: ListenerHandle,
    stream: ListenerStream,
}

/// Handle for shutting down a listener stream.
///
/// # Example
///
/// ```
/// use futures::stream::StreamExt;
/// use holochain_websocket::*;
/// use url2::url2;
///
/// #[tokio::main]
/// async fn main() {
///     let (listener_handle, mut listener_stream) = WebsocketListener::bind_with_handle(
///         url2!("ws://127.0.0.1:12345"),
///         std::sync::Arc::new(WebsocketConfig::default()),
///     )
///     .await
///     .unwrap();
///
///     tokio::spawn(async move { while let Some(Ok(_)) = listener_stream.next().await {} });
///     listener_handle.close();
/// }
/// ```
pub struct ListenerHandle {
    shutdown: Trigger,
    config: Arc<WebsocketConfig>,
    local_addr: Url2,
}

/// [`WebsocketSender`] and [`WebsocketReceiver`] for an active connection.
pub type Pair = (WebsocketSender, WebsocketReceiver);

/// New connection result returned from the [`ListenerStream`].
pub type ListenerItem = WebsocketResult<Pair>;

/// Stream of new connections.
pub type ListenerStream = BoxStream<'static, ListenerItem>;

impl WebsocketListener {
    /// Bind to a socket to accept incoming connections.
    pub async fn bind(addr: Url2, config: Arc<WebsocketConfig>) -> WebsocketResult<Self> {
        let (handle, stream) = Self::bind_with_handle(addr, config).await?;
        Ok(Self {
            handle,
            stream: stream.boxed(),
        })
    }

    #[instrument(skip(config, addr))]
    /// Same as [`WebsocketListener::bind`] but gives you a [`ListenerHandle`] to shutdown
    /// the listener and any open connections.
    pub async fn bind_with_handle(
        addr: Url2,
        config: Arc<WebsocketConfig>,
    ) -> WebsocketResult<(
        ListenerHandle,
        impl futures::stream::Stream<Item = ListenerItem>,
    )> {
        websocket_bind(addr, config).await
    }
    /// Shutdown the listener stream.
    pub fn close(self) {
        self.handle.close()
    }
    /// Get the url of the bound local listening socket.
    pub fn local_addr(&self) -> &Url2 {
        self.handle.local_addr()
    }
    /// Get the config associated with this listener.
    pub fn get_config(&self) -> Arc<WebsocketConfig> {
        self.handle.get_config()
    }

    /// Turn into a [`ListenerHandle`] and [`ListenerStream`].
    /// Can be done in place with [`WebsocketListener::bind_with_handle`]
    pub fn into_handle_and_stream(self) -> (ListenerHandle, ListenerStream) {
        (self.handle, self.stream)
    }
}

impl ListenerHandle {
    /// Shutdown the listener stream.
    pub fn close(self) {
        self.shutdown.cancel()
    }
    /// Get the url of the bound local listening socket.
    pub fn local_addr(&self) -> &Url2 {
        &self.local_addr
    }
    /// Get the config associated with this listener.
    pub fn get_config(&self) -> Arc<WebsocketConfig> {
        self.config.clone()
    }

    /// Close the listener when the future resolves to true.
    /// If the future returns false the listener will not be closed.
    /// ```
    /// # use futures::stream::StreamExt;
    /// # use holochain_websocket::*;
    /// # use url2::url2;
    /// #
    /// # #[tokio::main]
    /// # async fn main() {
    /// #     let (listener_handle, mut listener_stream) = WebsocketListener::bind_with_handle(
    /// #         url2!("ws://127.0.0.1:12345"),
    /// #         std::sync::Arc::new(WebsocketConfig::default()),
    /// #     )
    /// #     .await
    /// #     .unwrap();
    /// #
    ///  tokio::spawn(async move { while let Some(Ok(_)) = listener_stream.next().await {} });
    ///  let (tx, rx) = tokio::sync::oneshot::channel();
    ///  tokio::task::spawn(listener_handle.close_on(async move { rx.await.unwrap_or(true) }));
    ///  tx.send(true).unwrap();
    /// # }
    /// ```
    pub async fn close_on<F>(self, f: F)
    where
        F: std::future::Future<Output = bool>,
    {
        if f.await {
            self.close()
        }
    }
}

impl futures::stream::Stream for WebsocketListener {
    type Item = WebsocketResult<Pair>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let p = std::pin::Pin::new(&mut self.stream);
        futures::stream::Stream::poll_next(p, cx)
    }
}

async fn websocket_bind(
    addr: Url2,
    config: Arc<WebsocketConfig>,
) -> WebsocketResult<(
    ListenerHandle,
    impl futures::stream::Stream<Item = ListenerItem>,
)> {
    let addr = url_to_addr(&addr, config.scheme).await?;
    let socket = match &addr {
        SocketAddr::V4(_) => net2::TcpBuilder::new_v4()?,
        SocketAddr::V6(_) => net2::TcpBuilder::new_v6()?,
    }
    .reuse_address(true)?
    .bind(addr)?
    .listen(config.max_pending_connections as i32)?;
    socket.set_nonblocking(true)?;
    let local_addr = addr_to_url(socket.local_addr()?, config.scheme);
    let listener = tokio::net::TcpListener::from_std(socket)?;
    let listener_stream = tokio_stream::wrappers::TcpListenerStream::new(listener);

    // Setup proper shutdown
    let (shutdown, valve) = Valve::new();

    let buffered_listener = listener_stream
        .map_err(WebsocketError::from)
        .map_ok({
            let config = config.clone();
            let valve = valve.clone();
            move |socket_result| connect(config.clone(), socket_result, valve.clone())
        })
        .try_buffer_unordered(config.max_pending_connections);
    tracing::debug!(sever_listening_on = ?local_addr);

    let stream = valve.wrap(buffered_listener);

    let listener_handle = ListenerHandle {
        shutdown,
        config,
        local_addr,
    };
    Ok((listener_handle, stream))
}

#[instrument(skip(config, socket, valve))]
async fn connect(
    config: Arc<WebsocketConfig>,
    socket: tokio::net::TcpStream,
    valve: Valve,
) -> WebsocketResult<Pair> {
    // TODO: find alternative to set the keepalive
    // socket.set_keepalive(Some(std::time::Duration::from_secs(
    //     config.tcp_keepalive_s as u64,
    // )))?;
    tracing::debug!(
        message = "accepted incoming raw socket",
        remote_addr = %socket.peer_addr()?,
    );
    let socket = tokio_tungstenite::accept_async_with_config(
        socket,
        Some(tungstenite::protocol::WebSocketConfig {
            max_send_queue: Some(config.max_send_queue),
            max_message_size: Some(config.max_message_size),
            max_frame_size: Some(config.max_frame_size),
            ..Default::default()
        }),
    )
    .await
    .map_err(|e| Error::new(ErrorKind::Other, e))?;

    Websocket::create_ends(config, socket, valve)
}
