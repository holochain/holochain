use holochain_serialized_bytes::prelude::*;
pub use std::io::{Error, Result};
use std::sync::Arc;
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
    fn from_bytes(b: Vec<u8>) -> Result<Self> {
        let b = UnsafeBytes::from(b);
        let b = SerializedBytes::from(b);
        let b: WireMessage = b.try_into().map_err(Error::other)?;
        Ok(b)
    }

    fn request<S>(s: S) -> Result<(Message, u64)>
    where
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let s1 = SerializedBytes::try_from(s).map_err(Error::other)?;
        let s2 = Self::Request {
            id,
            data: UnsafeBytes::from(s1).into(),
        };
        let s3: SerializedBytes = s2.try_into().map_err(Error::other)?;
        Ok((Message::Binary(UnsafeBytes::from(s3).into()), id))
    }

    fn response<S>(id: u64, s: S) -> Result<Message>
    where
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

    fn signal<S>(s: S) -> Result<Message>
    where
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
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
    pub const DEFAULT: WebsocketConfig = WebsocketConfig {
        default_request_timeout: std::time::Duration::from_secs(60),
        max_message_size: 64 << 20,
        max_frame_size: 16 << 20,
    };

    pub(crate) fn to_tungstenite(
        &self,
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

#[derive(Clone)]
struct RMap(
    Arc<
        std::sync::Mutex<
            std::collections::HashMap<u64, tokio::sync::oneshot::Sender<SerializedBytes>>,
        >,
    >,
);

impl Default for RMap {
    fn default() -> Self {
        Self(Arc::new(std::sync::Mutex::new(
            std::collections::HashMap::default(),
        )))
    }
}

impl RMap {
    pub fn insert(&self, id: u64, sender: tokio::sync::oneshot::Sender<SerializedBytes>) {
        self.0.lock().unwrap().insert(id, sender);
    }

    pub fn remove(&self, id: u64) -> Option<tokio::sync::oneshot::Sender<SerializedBytes>> {
        self.0.lock().unwrap().remove(&id)
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
    fn close_if_err<R>(&self, r: Result<R>) -> Result<R> {
        match r {
            Err(err) => {
                self.0.lock().unwrap().take();
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
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
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
    Signal(D),

    /// Received a request from the remote.
    Request(D, WebsocketRespond),
}

/*
impl<D> ReceiveMessage<D>
where
    D: std::fmt::Debug,
    SerializedBytes: TryInto<D, Error = SerializedBytesError>,
{
    pub fn is_request(&self) -> bool {
        matches!(self, Self::Request(_, _))
    }
}
*/

/// Receive signals and requests from a websocket connection.
pub struct WebsocketReceiver(WsCoreSync);

impl WebsocketReceiver {
    /// Receive the next message.
    pub async fn recv<D>(&mut self) -> Result<ReceiveMessage<D>>
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
                    match WireMessage::from_bytes(msg)? {
                        WireMessage::Request { id, data } => {
                            let resp = WebsocketRespond {
                                id,
                                core: core_sync,
                            };
                            let data: D = SerializedBytes::from(UnsafeBytes::from(data))
                                .try_into()
                                .map_err(Error::other)?;
                            Ok(Some(ReceiveMessage::Request(data, resp)))
                        }
                        WireMessage::Response { id, data } => {
                            if let Some(sender) = core.rmap.remove(id) {
                                if let Some(data) = data {
                                    let data = SerializedBytes::from(UnsafeBytes::from(data));
                                    let _ = sender.send(data);
                                }
                            }
                            Ok(None)
                        }
                        WireMessage::Signal { data } => {
                            let data: D = SerializedBytes::from(UnsafeBytes::from(data))
                                .try_into()
                                .map_err(Error::other)?;
                            Ok(Some(ReceiveMessage::Signal(data)))
                        }
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
pub struct WebsocketSender(WsCoreSync, std::time::Duration);

impl WebsocketSender {
    /// Make a request of the remote using the default configured timeout.
    pub async fn request<S, R>(&self, s: S) -> Result<R>
    where
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
        R: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.request_timeout(s, self.1).await
    }

    /// Make a request of the remote.
    pub async fn request_timeout<S, R>(&self, s: S, timeout: std::time::Duration) -> Result<R>
    where
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
        R: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        let timeout_at = tokio::time::Instant::now() + timeout;

        use futures::sink::SinkExt;

        let (s, id) = WireMessage::request(s)?;

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
                let drop = D(core.rmap.clone(), id);
                core.rmap.insert(id, resp_s);

                tokio::time::timeout_at(timeout_at, async move {
                    core.send.lock().await.send(s).await.map_err(Error::other)?;

                    Ok(drop)
                })
                .await
                .map_err(Error::other)?
            })
            .await?;

        tokio::time::timeout_at(timeout_at, async {
            let resp = resp_r.await.map_err(|_| Error::other("ResponderDropped"))?;

            decode(&Vec::from(UnsafeBytes::from(resp))).map_err(Error::other)
        })
        .await
        .map_err(Error::other)?
    }

    /// Send a signal to the remote using the default configured timeout.
    pub async fn signal<S>(&self, s: S) -> Result<()>
    where
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        self.signal_timeout(s, self.1).await
    }

    /// Send a signal to the remote.
    pub async fn signal_timeout<S>(&self, s: S, timeout: std::time::Duration) -> Result<()>
    where
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
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let (sink, stream) = futures::stream::StreamExt::split(stream);

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
        WebsocketReceiver(core_recv),
    ))
}

/// Establish a new outgoing websocket connection to remote.
pub async fn connect(
    config: Arc<WebsocketConfig>,
    addr: std::net::SocketAddr,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let stream = tokio::net::TcpStream::connect(addr).await?;
    let url = format!("ws://{addr}");
    let (stream, _addr) =
        tokio_tungstenite::client_async_with_config(url, stream, Some(config.to_tungstenite()))
            .await
            .map_err(Error::other)?;
    split(stream, config.default_request_timeout)
}

/// A Holochain websocket listener.
pub struct WebsocketListener {
    config: Arc<WebsocketConfig>,
    listener: tokio::net::TcpListener,
}

impl WebsocketListener {
    /// Bind a new websocket listener.
    pub async fn bind<A: tokio::net::ToSocketAddrs>(
        config: Arc<WebsocketConfig>,
        addr: A,
    ) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind(addr).await?;

        let addr = listener.local_addr()?;
        tracing::info!(?addr, "WebsocketListener Listening");

        Ok(Self { config, listener })
    }

    /// Get the bound local address of this listener.
    pub fn local_addr(&self) -> Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Accept an incoming connection.
    pub async fn accept(&self) -> Result<(WebsocketSender, WebsocketReceiver)> {
        let (stream, addr) = self.listener.accept().await?;
        tracing::debug!(?addr, "Accept Incoming Websocket Connection");
        let stream =
            tokio_tungstenite::accept_async_with_config(stream, Some(self.config.to_tungstenite()))
                .await
                .map_err(Error::other)?;
        split(stream, self.config.default_request_timeout)
    }
}

#[cfg(test)]
mod test;
