#![deny(missing_docs)]
//! Holochain websocket support library.
//! This is currently a thin wrapper around tokio-tungstenite that
//! provides rpc-style request/responses via u64 message ids.

use holochain_serialized_bytes::prelude::*;
use holochain_types::websocket::AllowedOrigins;
pub use std::io::{Error, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::ToSocketAddrs;
use tokio::select;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::handshake::server::{Callback, ErrorResponse, Response};
use tokio_tungstenite::tungstenite::http::{HeaderMap, HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type")]
/// The messages actually sent over the wire by this library.
/// If you want to implement your own server or client you
/// will need this type or be able to serialize / deserialize it.
pub enum WireMessage {
    /// A message without a response.
    Signal {
        #[serde(with = "serde_bytes")]
        /// Actual bytes of the message serialized as [message pack](https://msgpack.org/).
        data: Vec<u8>,
    },

    /// An authentication message, sent by the client if the server requires it.
    Authenticate {
        #[serde(with = "serde_bytes")]
        /// Actual bytes of the message serialized as [message pack](https://msgpack.org/).
        data: Vec<u8>,
    },

    /// A request that requires a response.
    Request {
        /// The id of this request.
        id: u64,
        #[serde(with = "serde_bytes")]
        /// Actual bytes of the message serialized as [message pack](https://msgpack.org/).
        data: Vec<u8>,
    },

    /// The response to a request.
    Response {
        /// The id of the request that this response is for.
        id: u64,
        #[serde(with = "serde_bytes")]
        /// Actual bytes of the message serialized as [message pack](https://msgpack.org/).
        data: Option<Vec<u8>>,
    },
}

impl WireMessage {
    /// Deserialize a WireMessage.
    fn try_from_bytes(b: Vec<u8>) -> Result<Self> {
        let b = UnsafeBytes::from(b);
        let b = SerializedBytes::from(b);
        let b: WireMessage = b.try_into().map_err(Error::other)?;
        Ok(b)
    }

    /// Create a new authenticate message.
    fn authenticate<S>(s: S) -> Result<Message>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        let s1 = SerializedBytes::try_from(s).map_err(Error::other)?;
        let s2 = Self::Authenticate {
            data: UnsafeBytes::from(s1).into(),
        };
        let s3: SerializedBytes = s2.try_into().map_err(Error::other)?;
        Ok(Message::Binary(UnsafeBytes::from(s3).into()))
    }

    /// Create a new request message (with new unique msg id).
    fn request<S>(s: S) -> Result<(Message, u64)>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        tracing::trace!(?s, %id, "OutRequest");
        let s1 = SerializedBytes::try_from(s).map_err(Error::other)?;
        let s2 = Self::Request {
            id,
            data: UnsafeBytes::from(s1).into(),
        };
        let s3: SerializedBytes = s2.try_into().map_err(Error::other)?;
        Ok((Message::Binary(UnsafeBytes::from(s3).into()), id))
    }

    /// Create a new response message.
    fn response<S>(id: u64, s: S) -> Result<Message>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        let s1 = SerializedBytes::try_from(s).map_err(Error::other)?;
        let s2 = Self::Response {
            id,
            data: Some(UnsafeBytes::from(s1).into()),
        };
        let s3: SerializedBytes = s2.try_into().map_err(Error::other)?;
        Ok(Message::Binary(UnsafeBytes::from(s3).into()))
    }

    /// Create a new signal message.
    fn signal<S>(s: S) -> Result<Message>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        tracing::trace!(?s, "SendSignal");
        let s1 = SerializedBytes::try_from(s).map_err(Error::other)?;
        let s2 = Self::Signal {
            data: UnsafeBytes::from(s1).into(),
        };
        let s3: SerializedBytes = s2.try_into().map_err(Error::other)?;
        Ok(Message::Binary(UnsafeBytes::from(s3).into()))
    }
}

/// Websocket configuration struct.
#[derive(Clone, Debug)]
pub struct WebsocketConfig {
    /// Seconds after which the lib will stop tracking individual request ids.
    /// [default = 60 seconds]
    pub default_request_timeout: std::time::Duration,

    /// Maximum total message size of a websocket message. [default = 64M]
    pub max_message_size: usize,

    /// Maximum websocket frame size. [default = 16M]
    pub max_frame_size: usize,

    /// Allowed origins access control for a [WebsocketListener].
    /// Not used by the [WebsocketSender].
    pub allowed_origins: Option<AllowedOrigins>,
}

impl WebsocketConfig {
    /// The default client WebsocketConfig.
    pub const CLIENT_DEFAULT: WebsocketConfig = WebsocketConfig {
        default_request_timeout: std::time::Duration::from_secs(60),
        max_message_size: 64 << 20,
        max_frame_size: 16 << 20,
        allowed_origins: None,
    };

    /// The default listener WebsocketConfig.
    pub const LISTENER_DEFAULT: WebsocketConfig = WebsocketConfig {
        default_request_timeout: std::time::Duration::from_secs(60),
        max_message_size: 64 << 20,
        max_frame_size: 16 << 20,
        allowed_origins: Some(AllowedOrigins::Any),
    };

    /// Internal convert to tungstenite config.
    pub(crate) fn as_tungstenite(
        &self,
    ) -> tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
        tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
            max_message_size: Some(self.max_message_size),
            max_frame_size: Some(self.max_frame_size),
            ..Default::default()
        }
    }
}

struct RMapInner(
    pub std::collections::HashMap<u64, tokio::sync::oneshot::Sender<Result<SerializedBytes>>>,
);

impl Drop for RMapInner {
    fn drop(&mut self) {
        self.close();
    }
}

impl RMapInner {
    fn close(&mut self) {
        for (_, s) in self.0.drain() {
            let _ = s.send(Err(Error::other("ConnectionClosed")));
        }
    }
}

#[derive(Clone)]
struct RMap(Arc<std::sync::Mutex<RMapInner>>);

impl Default for RMap {
    fn default() -> Self {
        Self(Arc::new(std::sync::Mutex::new(RMapInner(
            std::collections::HashMap::default(),
        ))))
    }
}

impl RMap {
    pub fn close(&self) {
        if let Ok(mut lock) = self.0.lock() {
            lock.close();
        }
    }

    pub fn insert(&self, id: u64, sender: tokio::sync::oneshot::Sender<Result<SerializedBytes>>) {
        self.0.lock().unwrap().0.insert(id, sender);
    }

    pub fn remove(&self, id: u64) -> Option<tokio::sync::oneshot::Sender<Result<SerializedBytes>>> {
        self.0.lock().unwrap().0.remove(&id)
    }
}

type WsStream = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;
type WsSend =
    futures::stream::SplitSink<WsStream, tokio_tungstenite::tungstenite::protocol::Message>;
type WsSendSync = Arc<tokio::sync::Mutex<WsSend>>;
type WsRecv = futures::stream::SplitStream<WsStream>;
type WsRecvSync = Arc<tokio::sync::Mutex<WsRecv>>;

#[derive(Clone)]
struct WsCore {
    pub send: WsSendSync,
    pub recv: WsRecvSync,
    pub rmap: RMap,
    pub timeout: std::time::Duration,
}

#[derive(Clone)]
struct WsCoreSync(Arc<std::sync::Mutex<Option<WsCore>>>);

impl PartialEq for WsCoreSync {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl WsCoreSync {
    fn close(&self) {
        if let Some(core) = self.0.lock().unwrap().take() {
            core.rmap.close();
            tokio::task::spawn(async move {
                use futures::sink::SinkExt;
                let _ = core.send.lock().await.close().await;
            });
        }
    }

    fn close_if_err<R>(&self, r: Result<R>) -> Result<R> {
        match r {
            Err(err) => {
                self.close();
                Err(err)
            }
            Ok(res) => Ok(res),
        }
    }

    pub async fn exec<F, C, R>(&self, c: C) -> Result<R>
    where
        F: std::future::Future<Output = Result<R>>,
        C: FnOnce(WsCoreSync, WsCore) -> F,
    {
        let core = match self.0.lock().unwrap().as_ref() {
            Some(core) => core.clone(),
            None => return Err(Error::other("WebsocketClosed")),
        };
        self.close_if_err(c(self.clone(), core).await)
    }
}

/// Respond to an incoming request.
#[derive(PartialEq)]
pub struct WebsocketRespond {
    id: u64,
    core: WsCoreSync,
}

impl std::fmt::Debug for WebsocketRespond {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebsocketRespond")
            .field("id", &self.id)
            .finish()
    }
}

impl WebsocketRespond {
    /// Respond to an incoming request.
    pub async fn respond<S>(self, s: S) -> Result<()>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        tracing::trace!(?s, %self.id, "OutResponse");
        use futures::sink::SinkExt;
        self.core
            .exec(move |_, core| async move {
                tokio::time::timeout(core.timeout, async {
                    let s = WireMessage::response(self.id, s)?;
                    core.send.lock().await.send(s).await.map_err(Error::other)?;
                    Ok(())
                })
                .await
                .map_err(Error::other)?
            })
            .await
    }
}

/// Types of messages that can be received by a WebsocketReceiver.
#[derive(Debug, PartialEq)]
pub enum ReceiveMessage<D>
where
    D: std::fmt::Debug,
    SerializedBytes: TryInto<D, Error = SerializedBytesError>,
{
    /// Received a request to authenticate from the client.
    Authenticate(Vec<u8>),

    /// Received a signal from the remote.
    Signal(Vec<u8>),

    /// Received a request from the remote.
    Request(D, WebsocketRespond),
}

/// Receive signals and requests from a websocket connection.
/// Note, This receiver must be polled (recv()) for responses to requests
/// made on the Sender side to be received.
/// If this receiver is dropped, the sender side will also be closed.
pub struct WebsocketReceiver(
    WsCoreSync,
    std::net::SocketAddr,
    tokio::task::JoinHandle<()>,
);

impl Drop for WebsocketReceiver {
    fn drop(&mut self) {
        self.0.close();
        self.2.abort();
    }
}

impl WebsocketReceiver {
    fn new(core: WsCoreSync, addr: std::net::SocketAddr) -> Self {
        let core2 = core.clone();
        let ping_task = tokio::task::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                let core = core2.0.lock().unwrap().as_ref().cloned();
                if let Some(core) = core {
                    use futures::sink::SinkExt;
                    if core
                        .send
                        .lock()
                        .await
                        .send(Message::Ping(Vec::new()))
                        .await
                        .is_err()
                    {
                        core2.close();
                    }
                } else {
                    break;
                }
            }
        });
        Self(core, addr, ping_task)
    }

    /// Peer address.
    pub fn peer_addr(&self) -> std::net::SocketAddr {
        self.1
    }

    /// Receive the next message.
    pub async fn recv<D>(&mut self) -> Result<ReceiveMessage<D>>
    where
        D: std::fmt::Debug,
        SerializedBytes: TryInto<D, Error = SerializedBytesError>,
    {
        match self.recv_inner().await {
            Err(err) => {
                tracing::warn!(?err, "WebsocketReceiver Error");
                Err(err)
            }
            Ok(msg) => Ok(msg),
        }
    }

    async fn recv_inner<D>(&mut self) -> Result<ReceiveMessage<D>>
    where
        D: std::fmt::Debug,
        SerializedBytes: TryInto<D, Error = SerializedBytesError>,
    {
        use futures::sink::SinkExt;
        use futures::stream::StreamExt;
        loop {
            if let Some(result) = self
                .0
                .exec(move |core_sync, core| async move {
                    let msg = core
                        .recv
                        .lock()
                        .await
                        .next()
                        .await
                        .ok_or(Error::other("ReceiverClosed"))?
                        .map_err(Error::other)?;
                    let msg = match msg {
                        Message::Text(s) => s.into_bytes(),
                        Message::Binary(b) => b,
                        Message::Ping(b) => {
                            core.send
                                .lock()
                                .await
                                .send(Message::Pong(b))
                                .await
                                .map_err(Error::other)?;
                            return Ok(None);
                        }
                        Message::Pong(_) => return Ok(None),
                        Message::Close(frame) => {
                            return Err(Error::other(format!("ReceivedCloseFrame: {frame:?}")));
                        }
                        Message::Frame(_) => return Err(Error::other("UnexpectedRawFrame")),
                    };
                    match WireMessage::try_from_bytes(msg)? {
                        WireMessage::Authenticate { data } => {
                            Ok(Some(ReceiveMessage::Authenticate(data)))
                        }
                        WireMessage::Request { id, data } => {
                            let resp = WebsocketRespond {
                                id,
                                core: core_sync,
                            };
                            let data: D = SerializedBytes::from(UnsafeBytes::from(data))
                                .try_into()
                                .map_err(Error::other)?;
                            tracing::trace!(?data, %id, "InRequest");
                            Ok(Some(ReceiveMessage::Request(data, resp)))
                        }
                        WireMessage::Response { id, data } => {
                            if let Some(sender) = core.rmap.remove(id) {
                                if let Some(data) = data {
                                    let data = SerializedBytes::from(UnsafeBytes::from(data));
                                    tracing::trace!(%id, ?data, "InResponse");
                                    let _ = sender.send(Ok(data));
                                }
                            }
                            Ok(None)
                        }
                        WireMessage::Signal { data } => Ok(Some(ReceiveMessage::Signal(data))),
                    }
                })
                .await?
            {
                return Ok(result);
            }
        }
    }
}

/// Send requests and signals to the remote end of this websocket connection.
/// Note, this receiver side must be polled (recv()) for responses to requests
/// made on this sender to be received.
#[derive(Clone)]
pub struct WebsocketSender(WsCoreSync, std::time::Duration);

impl WebsocketSender {
    /// Authenticate with the remote using the default configured timeout.
    pub async fn authenticate<S>(&self, s: S) -> Result<()>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        self.authenticate_timeout(s, self.1).await
    }

    /// Authenticate with the remote.
    pub async fn authenticate_timeout<S>(&self, s: S, timeout: std::time::Duration) -> Result<()>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        use futures::sink::SinkExt;
        self.0
            .exec(move |_, core| async move {
                tokio::time::timeout(timeout, async {
                    let s = WireMessage::authenticate(s)?;
                    core.send.lock().await.send(s).await.map_err(Error::other)?;
                    Ok(())
                })
                .await
                .map_err(Error::other)?
            })
            .await
    }

    /// Make a request of the remote using the default configured timeout.
    /// Note, this receiver side must be polled (recv()) for responses to
    /// requests made on this sender to be received.
    pub async fn request<S, R>(&self, s: S) -> Result<R>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
        R: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.request_timeout(s, self.1).await
    }

    /// Make a request of the remote.
    pub async fn request_timeout<S, R>(&self, s: S, timeout: std::time::Duration) -> Result<R>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
        R: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        let timeout_at = tokio::time::Instant::now() + timeout;

        use futures::sink::SinkExt;

        let (s, id) = WireMessage::request(s)?;

        /// Drop helper to remove our response callback if we timeout.
        struct D(RMap, u64);

        impl Drop for D {
            fn drop(&mut self) {
                self.0.remove(self.1);
            }
        }

        let (resp_s, resp_r) = tokio::sync::oneshot::channel();

        let _drop = self
            .0
            .exec(move |_, core| async move {
                // create the drop helper
                let drop = D(core.rmap.clone(), id);

                // register the response callback
                core.rmap.insert(id, resp_s);

                tokio::time::timeout_at(timeout_at, async move {
                    // send the actual message
                    core.send.lock().await.send(s).await.map_err(Error::other)?;

                    Ok(drop)
                })
                .await
                .map_err(Error::other)?
            })
            .await?;

        // do the remainder outside the 'exec' because we don't actually
        // want to close the connection down if an individual response is
        // not returned... that is separate from the connection no longer
        // being viable. (but we still want it to timeout at the same point)
        tokio::time::timeout_at(timeout_at, async {
            // await the response
            let resp = resp_r
                .await
                .map_err(|_| Error::other("ResponderDropped"))??;

            // decode the response
            let res = decode(&Vec::from(UnsafeBytes::from(resp))).map_err(Error::other)?;
            tracing::trace!(?res, %id, "OutRequestResponse");
            Ok(res)
        })
        .await
        .map_err(Error::other)?
    }

    /// Send a signal to the remote using the default configured timeout.
    pub async fn signal<S>(&self, s: S) -> Result<()>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        self.signal_timeout(s, self.1).await
    }

    /// Send a signal to the remote.
    pub async fn signal_timeout<S>(&self, s: S, timeout: std::time::Duration) -> Result<()>
    where
        S: std::fmt::Debug,
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        use futures::sink::SinkExt;
        self.0
            .exec(move |_, core| async move {
                tokio::time::timeout(timeout, async {
                    let s = WireMessage::signal(s)?;
                    core.send.lock().await.send(s).await.map_err(Error::other)?;
                    Ok(())
                })
                .await
                .map_err(Error::other)?
            })
            .await
    }
}

fn split(
    stream: WsStream,
    timeout: std::time::Duration,
    peer_addr: std::net::SocketAddr,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let (sink, stream) = futures::stream::StreamExt::split(stream);

    // Q: Why do we split the parts only to seemingly put them back together?
    // A: They are in separate tokio mutexes, so we can still receive
    //    and send at the same time in separate tasks, but being in the same
    //    WsCore(Sync) lets us close them both at the same time if either
    //    one errors.
    let core = WsCore {
        send: Arc::new(tokio::sync::Mutex::new(sink)),
        recv: Arc::new(tokio::sync::Mutex::new(stream)),
        rmap: RMap::default(),
        timeout,
    };

    let core_send = WsCoreSync(Arc::new(std::sync::Mutex::new(Some(core))));
    let core_recv = core_send.clone();

    Ok((
        WebsocketSender(core_send, timeout),
        WebsocketReceiver::new(core_recv, peer_addr),
    ))
}

/// Establish a new outgoing websocket connection to remote.
pub async fn connect(
    config: Arc<WebsocketConfig>,
    request: impl Into<ConnectRequest>,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let request = request.into();
    let stream = tokio::net::TcpStream::connect(request.addr).await?;
    let peer_addr = stream.peer_addr()?;
    let (stream, _addr) = tokio_tungstenite::client_async_with_config(
        request.into_client_request()?,
        stream,
        Some(config.as_tungstenite()),
    )
    .await
    .map_err(Error::other)?;
    split(stream, config.default_request_timeout, peer_addr)
}

/// A request to connect to a websocket server.
pub struct ConnectRequest {
    addr: std::net::SocketAddr,
    headers: HeaderMap<HeaderValue>,
}

impl From<std::net::SocketAddr> for ConnectRequest {
    fn from(addr: std::net::SocketAddr) -> Self {
        Self::new(addr)
    }
}

impl ConnectRequest {
    /// Create a new [ConnectRequest].
    pub fn new(addr: std::net::SocketAddr) -> Self {
        let mut cr = ConnectRequest {
            addr,
            headers: HeaderMap::new(),
        };

        // Set a default Origin so that the connection request will be allowed by default when the listener is
        // using `Any` as the allowed origin.
        cr.headers.insert(
            "Origin",
            HeaderValue::from_str("holochain_websocket").expect("Invalid Origin value"),
        );

        cr
    }

    /// Try to set a header on this request.
    ///
    /// Errors if the value is invalid. See [HeaderValue::from_str].
    pub fn try_set_header(mut self, name: &'static str, value: &str) -> Result<Self> {
        self.headers
            .insert(name, HeaderValue::from_str(value).map_err(Error::other)?);
        Ok(self)
    }

    fn into_client_request(
        self,
    ) -> Result<impl tokio_tungstenite::tungstenite::client::IntoClientRequest + Unpin> {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut req =
            String::into_client_request(format!("ws://{}", self.addr)).map_err(Error::other)?;
        for (name, value) in self.headers {
            if let Some(name) = name {
                req.headers_mut().insert(name, value);
            } else {
                tracing::warn!("Dropping invalid header");
            }
        }
        Ok(req)
    }

    #[cfg(test)]
    pub(crate) fn clear_headers(mut self) -> Self {
        self.headers.clear();

        self
    }
}

// TODO async_trait still needed for dynamic dispatch https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits.html#dynamic-dispatch
#[async_trait::async_trait]
trait TcpListener: Send + Sync {
    async fn accept(&self) -> Result<(tokio::net::TcpStream, SocketAddr)>;

    fn local_addrs(&self) -> Result<Vec<SocketAddr>>;
}

#[async_trait::async_trait]
impl TcpListener for tokio::net::TcpListener {
    async fn accept(&self) -> Result<(tokio::net::TcpStream, SocketAddr)> {
        self.accept().await
    }

    fn local_addrs(&self) -> Result<Vec<SocketAddr>> {
        Ok(vec![self.local_addr()?])
    }
}

struct DualStackListener {
    v4: tokio::net::TcpListener,
    v6: tokio::net::TcpListener,
}

#[async_trait::async_trait]
impl TcpListener for DualStackListener {
    async fn accept(&self) -> Result<(tokio::net::TcpStream, SocketAddr)> {
        let (stream, addr) = select! {
            res = self.v4.accept() => res?,
            res = self.v6.accept() => res?,
        };
        Ok((stream, addr))
    }

    fn local_addrs(&self) -> Result<Vec<SocketAddr>> {
        Ok(vec![self.v4.local_addr()?, self.v6.local_addr()?])
    }
}

/// A Holochain websocket listener.
pub struct WebsocketListener {
    config: Arc<WebsocketConfig>,
    access_control: Arc<AllowedOrigins>,
    listener: Box<dyn TcpListener>,
}

impl Drop for WebsocketListener {
    fn drop(&mut self) {
        tracing::info!("WebsocketListenerDrop");
    }
}

impl WebsocketListener {
    /// Bind a new websocket listener.
    pub async fn bind(config: Arc<WebsocketConfig>, addr: impl ToSocketAddrs) -> Result<Self> {
        let access_control = Arc::new(config.allowed_origins.clone().ok_or_else(|| {
            Error::other("WebsocketListener requires access control to be set in the config")
        })?);

        let listener = tokio::net::TcpListener::bind(addr).await?;

        let addr = listener.local_addr()?;
        tracing::info!(?addr, "WebsocketListener Listening");

        Ok(Self {
            config,
            access_control,
            listener: Box::new(listener),
        })
    }

    /// Bind a new websocket listener on the same port using a v4 and a v6 socket.
    pub async fn dual_bind(
        config: Arc<WebsocketConfig>,
        addr_v4: impl Into<SocketAddr>,
        addr_v6: impl Into<SocketAddr>,
    ) -> Result<Self> {
        let access_control = Arc::new(config.allowed_origins.clone().ok_or_else(|| {
            Error::other("WebsocketListener requires access control to be set in the config")
        })?);

        let mut addr_v4 = addr_v4.into();
        let addr_v6 = addr_v6.into();

        if !addr_v4.is_ipv4() {
            return Err(Error::other("Expected an IPv4 address"));
        }

        if !addr_v6.is_ipv6() {
            return Err(Error::other("Expected an IPv6 address"));
        }

        // Note that tokio binds to the stack matching the address type so we can re-use the port
        // without needing to create the sockets ourselves to configure this.
        let v6_listener = tokio::net::TcpListener::bind(addr_v6).await?;
        addr_v4.set_port(v6_listener.local_addr()?.port());

        let v4_listener = tokio::net::TcpListener::bind(addr_v4).await?;

        let listener = DualStackListener {
            v4: v4_listener,
            v6: v6_listener,
        };

        let addr = listener.v4.local_addr()?;
        tracing::info!(?addr, "WebsocketListener listening");

        let addr = listener.v6.local_addr()?;
        tracing::info!(?addr, "WebsocketListener listening");

        Ok(Self {
            config,
            access_control,
            listener: Box::new(listener),
        })
    }

    /// Get the bound local address of this listener.
    pub fn local_addrs(&self) -> Result<Vec<std::net::SocketAddr>> {
        self.listener.local_addrs()
    }

    /// Accept an incoming connection.
    pub async fn accept(&self) -> Result<(WebsocketSender, WebsocketReceiver)> {
        let (stream, addr) = self.listener.accept().await?;
        tracing::debug!(?addr, "Accept Incoming Websocket Connection");
        let stream = tokio_tungstenite::accept_hdr_async_with_config(
            stream,
            ConnectCallback {
                allowed_origin: self.access_control.clone(),
            },
            Some(self.config.as_tungstenite()),
        )
        .await
        .map_err(Error::other)?;
        split(stream, self.config.default_request_timeout, addr)
    }
}

struct ConnectCallback {
    allowed_origin: Arc<AllowedOrigins>,
}

impl Callback for ConnectCallback {
    fn on_request(
        self,
        request: &Request,
        response: Response,
    ) -> std::result::Result<Response, ErrorResponse> {
        tracing::trace!(
            "Checking incoming websocket connection request with allowed origin {:?}: {:?}",
            self.allowed_origin,
            request.headers()
        );
        match request
            .headers()
            .get("Origin")
            .and_then(|v| v.to_str().ok())
        {
            Some(origin) => {
                if self.allowed_origin.is_allowed(origin) {
                    Ok(response)
                } else {
                    tracing::warn!("Rejecting websocket connection request with disallowed `Origin` header: {:?}", request);
                    let allowed_origin: String = self.allowed_origin.as_ref().clone().into();
                    match HeaderValue::from_str(&allowed_origin) {
                        Ok(allowed_origin) => {
                            let mut err_response = ErrorResponse::new(None);
                            *err_response.status_mut() = StatusCode::BAD_REQUEST;
                            err_response
                                .headers_mut()
                                .insert("Access-Control-Allow-Origin", allowed_origin);
                            Err(err_response)
                        }
                        Err(_) => {
                            // Shouldn't be possible to get here, the listener should be configured to require an origin
                            let mut err_response = ErrorResponse::new(Some(
                                "Invalid listener configuration for `Origin`".to_string(),
                            ));
                            *err_response.status_mut() = StatusCode::BAD_REQUEST;
                            Err(err_response)
                        }
                    }
                }
            }
            None => {
                tracing::warn!(
                    "Rejecting websocket connection request with missing `Origin` header: {:?}",
                    request
                );
                let mut err_response =
                    ErrorResponse::new(Some("Missing `Origin` header".to_string()));
                *err_response.status_mut() = StatusCode::BAD_REQUEST;
                Err(err_response)
            }
        }
    }
}

#[cfg(test)]
mod test;
