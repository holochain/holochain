#![deny(missing_docs)]
//! QUIC transport module for kitsune-p2p

/// Re-exported dependencies.
pub mod dependencies {
    pub use ::kitsune_p2p_types;
    pub use ::quinn;
}

use kitsune_p2p_types::dependencies::url2::*;
use kitsune_p2p_types::metric_task;
use kitsune_p2p_types::transport::TransportResult;
use std::net::SocketAddr;

const SCHEME: &str = "kitsune-quic";

/// internal helper convert urls to socket addrs for binding / connection
pub(crate) async fn url_to_addr(url: &Url2, scheme: &str) -> TransportResult<SocketAddr> {
    if url.scheme() != scheme || url.host_str().is_none() || url.port().is_none() {
        return Err(format!(
            "invalid input. got: '{}', expected: '{}://host:port'",
            scheme, url
        )
        .into());
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

    Err(format!("could not parse '{}', as 'host:port'", rendered).into())
}

mod config;
pub use config::*;

mod listener;
pub use listener::*;

mod test;
