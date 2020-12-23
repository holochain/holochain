//! internal websocket utility types and code

use crate::*;

/// Implements both sides of TryFrom SerializedBytes for the passed in item.
/// See holochain_serialized_bytes::holochain_serial! macro.
/// This is similar, but makes use of std::io::Error for the error type.
#[macro_export]
macro_rules! try_from_serialized_bytes {
    ($s:ident) => {
        impl ::std::convert::TryFrom<$s> for ::holochain_serialized_bytes::SerializedBytes {
            type Error = ::std::io::Error;

            fn try_from(t: $s) -> ::std::io::Result<::holochain_serialized_bytes::SerializedBytes> {
                ::holochain_serialized_bytes::encode(&t)
                    .map_err(|e| ::std::io::Error::new(::std::io::ErrorKind::Other, e))
                    .map(|bytes| {
                        ::holochain_serialized_bytes::SerializedBytes::from(
                            ::holochain_serialized_bytes::UnsafeBytes::from(bytes),
                        )
                    })
            }
        }

        impl ::std::convert::TryFrom<::holochain_serialized_bytes::SerializedBytes> for $s {
            type Error = ::std::io::Error;

            fn try_from(t: ::holochain_serialized_bytes::SerializedBytes) -> ::std::io::Result<$s> {
                ::holochain_serialized_bytes::decode(t.bytes())
                    .map_err(|e| ::std::io::Error::new(::std::io::ErrorKind::Other, e))
            }
        }
    };
}

/// not sure if we should expose this or not
/// this is the actual wire message that is sent over the websocket.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub(crate) enum WireMessage {
    Signal {
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    Request {
        id: String,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    Response {
        id: String,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
}
try_from_serialized_bytes!(WireMessage);

#[cfg(test)]
pub(crate) fn init_tracing() {
    observability::test_run().unwrap();
}

/// internal socket type
pub(crate) type RawSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

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
