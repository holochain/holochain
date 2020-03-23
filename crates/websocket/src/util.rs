//! internal websocket utility types and code

use crate::*;

const SCHEME: &str = "ws";

/// internal socket type
pub(crate) type RawSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

/// internal message sender type
pub(crate) type RawSender = tokio::sync::mpsc::Sender<tungstenite::Message>;

/// internal helper to convert addrs to urls
pub(crate) fn addr_to_url(a: SocketAddr) -> Url2 {
    url2!("{}://{}", SCHEME, a)
}

/// internal helper convert urls to socket addrs for binding / connection
pub(crate) async fn url_to_addr(url: &Url2) -> Result<SocketAddr> {
    if url.scheme() != SCHEME || url.host_str().is_none() || url.port().is_none() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("got: '{}', expected: '{}://host:port'", SCHEME, url),
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
