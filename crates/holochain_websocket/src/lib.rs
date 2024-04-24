#![deny(missing_docs)]
//! Holochain websocket support library.
//! This is currently a thin wrapper around tokio-tungstenite that
//! provides rpc-style request/responses via u64 message ids.

use holochain_serialized_bytes::prelude::*;
use std::io::ErrorKind;
pub use std::io::{Error, Result};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::sync::Arc;
use tokio::net::ToSocketAddrs;
use tokio::select;
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type")]
/// The messages actually sent over the wire by this library.
/// If you want to impliment your own server or client you
/// will need this type or be able to serialize / deserialize it.
pub enum WireMessage {
    /// A message without a response.
    Signal {
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
#[derive(Clone, Copy, Debug)]
pub struct WebsocketConfig {
    /// Seconds after which the lib will stop tracking individual request ids.
    /// [default = 60 seconds]
    pub default_request_timeout: std::time::Duration,

    /// Maximum total message size of a websocket message. [default = 64M]
    pub max_message_size: usize,

    /// Maximum websocket frame size. [default = 16M]
    pub max_frame_size: usize,
}

impl WebsocketConfig {
    /// The default WebsocketConfig.
    pub const DEFAULT: WebsocketConfig = WebsocketConfig {
        default_request_timeout: std::time::Duration::from_secs(60),
        max_message_size: 64 << 20,
        max_frame_size: 16 << 20,
    };

    /// Internal convert to tungstenite config.
    pub(crate) fn to_tungstenite(
        self,
    ) -> tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
        tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
            max_message_size: Some(self.max_message_size),
            max_frame_size: Some(self.max_frame_size),
            ..Default::default()
        }
    }
}

impl Default for WebsocketConfig {
    fn default() -> Self {
        WebsocketConfig::DEFAULT
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
    addr: std::net::SocketAddr,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let stream = tokio::net::TcpStream::connect(addr).await?;
    let peer_addr = stream.peer_addr()?;
    let url = format!("ws://{addr}");
    let (stream, _addr) =
        tokio_tungstenite::client_async_with_config(url, stream, Some(config.to_tungstenite()))
            .await
            .map_err(Error::other)?;
    split(stream, config.default_request_timeout, peer_addr)
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
        let listener = tokio::net::TcpListener::bind(addr).await?;

        let addr = listener.local_addr()?;
        tracing::info!(?addr, "WebsocketListener Listening");

        Ok(Self {
            config,
            listener: Box::new(listener),
        })
    }

    /// Bind a new websocket listener on the same port using a v4 and a v6 socket.
    ///
    /// If the port is 0, then the OS will be allowed to pick a port for IPv6. This function will
    /// then try to bind to the same port for IPv4. If the OS picks a port that is not available for
    /// IPv4, then the function will retry binding IPv6 to get a new port and see if that is
    /// available for IPv4. If this fails after 5 retries, then an error will be returned.
    ///
    /// If either IPv4 or IPv6 is disabled, then the function will fall back to binding to the
    /// available stack. An info message will be logged to let the user know that one interface was
    /// unavailable, but this is likely intentional or expected in the user's environment, so it will
    /// not be treated as an error that should prevent the listener from starting.
    ///
    /// Note: The interface fallback behaviour can be tested manually on Linux by running:
    /// `echo 1 | sudo tee /proc/sys/net/ipv6/conf/lo/disable_ipv6`
    /// and then trying to start Holochain with info logging enabled. You can undo the change with:
    /// `echo 0 | sudo tee /proc/sys/net/ipv6/conf/lo/disable_ipv6`.
    pub async fn dual_bind(
        config: Arc<WebsocketConfig>,
        addr_v4: SocketAddrV4,
        addr_v6: SocketAddrV6,
    ) -> Result<Self> {
        let addr_v6: SocketAddr = addr_v6.into();
        let mut addr_v4: SocketAddr = addr_v4.into();

        // The point of dual_bind is to bind to the same port on both v4 and v6
        if addr_v6.port() != 0 && addr_v6.port() != addr_v4.port() {
            return Err(Error::other(
                "dual_bind requires the same port for IPv4 and IPv6",
            ));
        }

        // Note that tokio binds to the stack matching the address type, so we can re-use the port
        // without needing to create the sockets ourselves to configure this.

        let mut listener: Option<DualStackListener> = None;
        for _ in 0..5 {
            let v6_listener = match tokio::net::TcpListener::bind(addr_v6).await {
                Ok(l) => l,
                // This is the error code that *should* be returned if IPv6 is disabled
                Err(e) if e.kind() == ErrorKind::AddrNotAvailable => {
                    tracing::info!(?e, "Failed to bind IPv6 listener because IPv6 appears to be disabled, falling back to IPv4 only");
                    return Self::bind(config, addr_v4).await;
                }
                Err(e) => {
                    return Err(e);
                }
            };

            addr_v4.set_port(v6_listener.local_addr()?.port());

            let v4_listener = match tokio::net::TcpListener::bind(addr_v4).await {
                Ok(l) => l,
                // This is the error code that *should* be returned if IPv4 is disabled
                Err(e) if e.kind() == ErrorKind::AddrNotAvailable => {
                    tracing::info!(?e, "Failed to bind IPv4 listener because IPv4 appears to be disabled, falling back to IPv6 only");
                    // No need to re-bind the v6 listener, it's already bound. Just create a new Self
                    // from the v6 listener and return it.
                    return Ok(Self {
                        config,
                        listener: Box::new(v6_listener),
                    });
                }
                // If the port for IPv6 was selected by the OS but it isn't available for IPv4, retry and let the OS pick a new port for IPv6
                // and hopefully it will be available for IPv4.
                Err(e) if addr_v6.port() == 0 && e.kind() == ErrorKind::AddrInUse => {
                    tracing::warn!(?e, "Failed to bind the same port for IPv4 that was selected for IPv6, retrying with a new port");
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            };

            listener = Some(DualStackListener {
                v4: v4_listener,
                v6: v6_listener,
            });
            break;
        }

        // Gave up after a few retries, there's no point in continuing forever because there might be
        // something wrong that the logic above isn't accounting for.
        let listener = listener.ok_or_else(|| {
            Error::other("Failed to bind listener to IPv4 and IPv6 interfaces after 5 retries")
        })?;

        let addr = listener.v4.local_addr()?;
        tracing::info!(?addr, "WebsocketListener listening");

        let addr = listener.v6.local_addr()?;
        tracing::info!(?addr, "WebsocketListener listening");

        Ok(Self {
            config,
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
        let stream =
            tokio_tungstenite::accept_async_with_config(stream, Some(self.config.to_tungstenite()))
                .await
                .map_err(Error::other)?;
        split(stream, self.config.default_request_timeout, addr)
    }
}

#[cfg(test)]
mod test;
