//! internal websocket utility types and code

use crate::*;

use serde::{Deserialize, Serialize};

/// internal socket type
pub(crate) type RawSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum Message {
    Signal { data: Vec<u8> },
    Request { id: String, data: Vec<u8> },
    Response { id: String, data: Vec<u8> },
}

impl Message {
    pub(crate) fn clone_id(&self) -> Option<String> {
        match self {
            Message::Signal { .. } => None,
            Message::Request { id, .. } => Some(id.clone()),
            Message::Response { id, .. } => Some(id.clone()),
        }
    }
}

impl std::convert::TryFrom<Message> for SerializedBytes {
    type Error = Error;

    fn try_from(t: Message) -> Result<SerializedBytes> {
        holochain_serialized_bytes::to_vec_named(&t)
            .map_err(|e| Error::new(ErrorKind::Other, e))
            .map(|bytes| SerializedBytes::from(UnsafeBytes::from(bytes)))
    }
}

impl std::convert::TryFrom<SerializedBytes> for Message {
    type Error = Error;

    fn try_from(t: SerializedBytes) -> Result<Message> {
        holochain_serialized_bytes::from_read_ref(t.bytes())
            .map_err(|e| Error::new(ErrorKind::Other, e))
    }
}

/// internal message sender type
pub(crate) type RawSender = tokio::sync::mpsc::Sender<(
    Message,
    Option<tokio::sync::oneshot::Sender<Result<Vec<u8>>>>,
)>;

/// internal message receiver type
pub(crate) type RawReceiver = tokio::sync::mpsc::Receiver<(
    Message,
    Option<tokio::sync::oneshot::Sender<Result<Vec<u8>>>>,
)>;

/// internal helper to convert addrs to urls
pub(crate) fn addr_to_url(a: SocketAddr, scheme: &str) -> Url2 {
    url2!("{}://{}", scheme, a)
}

/// internal helper convert urls to socket addrs for binding / connection
pub(crate) async fn url_to_addr(url: &Url2, scheme: &str) -> Result<SocketAddr> {
    if url.scheme() != scheme || url.host_str().is_none() || url.port().is_none() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("got: '{}', expected: '{}://host:port'", scheme, url),
        ));
    }

    let rendered = format!("{}:{}", url.host_str().unwrap(), url.port().unwrap());

    if let Ok(mut iter) = tokio::net::lookup_host(rendered.clone()).await {
        let mut tmp = iter.next();
        let mut fallback = None;
        loop {
            if tmp.is_none() {
                break;
            }

            if tmp.as_ref().unwrap().is_ipv4() {
                return Ok(tmp.unwrap());
            }

            fallback = tmp;
            tmp = iter.next();
        }
        if let Some(addr) = fallback {
            return Ok(addr);
        }
    }

    Err(Error::new(
        ErrorKind::InvalidInput,
        format!("could not parse '{}', as 'host:port'", rendered),
    ))
}
