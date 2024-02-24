pub use std::io::{Error, Result};
use std::sync::Arc;
use holochain_serialized_bytes::prelude::*;
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

    pub(crate) fn to_tungstenite(&self) -> tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
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

type WsStream = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;
type WsSend = futures::stream::SplitSink<WsStream, tokio_tungstenite::tungstenite::protocol::Message>;
type WsSendSync = Arc<tokio::sync::Mutex<WsSend>>;
type WsRecv = futures::stream::SplitStream<WsStream>;
type WsRecvSync = Arc<tokio::sync::Mutex<WsRecv>>;
type WsCore = Arc<std::sync::Mutex<Option<(WsSendSync, WsRecvSync)>>>;

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
    Request(D, ()),
}

/// Receive signals and requests from a websocket connection.
pub struct WebsocketReceiver(WsCore);

impl WebsocketReceiver {
    fn get_receiver(&self) -> Result<WsRecvSync> {
        match self.0.lock().unwrap().as_ref() {
            Some(core) => {
                Ok(core.1.clone())
            }
            None => Err(Error::other("ReceiverClosed")),
        }
    }

    /// Receive the next message.
    pub async fn recv<D>(&mut self) -> Result<ReceiveMessage<D>>
    where
        D: std::fmt::Debug,
        SerializedBytes: TryInto<D, Error = SerializedBytesError>,
    {
        use futures::stream::StreamExt;
        let receiver = self.get_receiver()?;
        let msg = receiver.lock().await.next().await.ok_or(Error::other("ReceiverClosed"))?.map_err(Error::other)?;
        let msg = match msg {
            Message::Text(s) => s.into_bytes(),
            Message::Binary(b) => b,
            oth => panic!("unhandled: {oth:?}"),
        };
        match WireMessage::from_bytes(msg)? {
            WireMessage::Signal { data } => {
                let data: D = SerializedBytes::from(
                    UnsafeBytes::from(data)
                ).try_into().map_err(Error::other)?;
                Ok(ReceiveMessage::Signal(data))
            }
            oth => panic!("unhandled: {oth:?}"),
        }
    }
}

/// Send requests and signals to the remote end of this websocket connection.
pub struct WebsocketSender(WsCore);

impl WebsocketSender {
    fn get_sender(&self) -> Result<WsSendSync> {
        match self.0.lock().unwrap().as_ref() {
            Some(core) => {
                Ok(core.0.clone())
            }
            None => Err(Error::other("SenderClosed")),
        }
    }

    /// Send a signal to the remote.
    pub async fn signal_timeout<S>(&self, s: S, timeout: std::time::Duration) -> Result<()>
    where
        SerializedBytes: TryFrom<S, Error = SerializedBytesError>,
    {
        use futures::sink::SinkExt;
        tokio::time::timeout(timeout, async {
            let s = WireMessage::signal(s)?;
            let sender = self.get_sender()?;
            sender.lock().await.send(s).await.map_err(Error::other)?;
            Ok(())
        }).await.map_err(Error::other)?
    }
}

fn split(
    stream: WsStream,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let (sink, stream) = futures::stream::StreamExt::split(stream);
    let core_send = Arc::new(std::sync::Mutex::new(Some((
        Arc::new(tokio::sync::Mutex::new(sink)),
        Arc::new(tokio::sync::Mutex::new(stream)),
    ))));
    let core_recv = core_send.clone();
    Ok((WebsocketSender(core_send), WebsocketReceiver(core_recv)))
}

/// Establish a new outgoing websocket connection to remote.
pub async fn connect(
    config: Arc<WebsocketConfig>,
    addr: std::net::SocketAddr,
) -> Result<(WebsocketSender, WebsocketReceiver)> {
    let stream = tokio::net::TcpStream::connect(addr).await?;
    let url = format!("ws://{addr}");
    let (stream, _addr) = tokio_tungstenite::client_async_with_config(
        url, stream, Some(config.to_tungstenite())
    ).await.map_err(Error::other)?;
    split(stream)
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

        Ok(Self {
            config,
            listener,
        })
    }

    /// Get the bound local address of this listener.
    pub fn local_addr(&self) -> Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Accept an incoming connection.
    pub async fn accept(&self) -> Result<(WebsocketSender, WebsocketReceiver)> {
        let (stream, addr) = self.listener.accept().await?;
        tracing::debug!(?addr, "Accept Incoming Websocket Connection");
        let stream = tokio_tungstenite::accept_async_with_config(
            stream,
            Some(self.config.to_tungstenite()),
        ).await.map_err(Error::other)?;
        split(stream)
    }
}

#[cfg(test)]
mod test;
