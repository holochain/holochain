//! internal websocket utility types and code

use std::net::SocketAddr;

use url2::{url2, Url2};

use std::io::{Error, ErrorKind, Result};

pub(crate) type ToFromSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

/// Amount of time to spend waiting for channels to empty before forcing them to close.
pub(crate) const CLOSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

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
