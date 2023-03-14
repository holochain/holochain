fn main() {}
/*
use holochain_trace::{span_context, OpenSpanExt};
use std::{env, error::Error, net::SocketAddr};
use tokio::net::UdpSocket;
use tracing::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    holochain_trace::test_run_open().ok();
    let remote_addr: SocketAddr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".into())
        .parse()?;
    let local_addr: SocketAddr = if remote_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    }
    .parse()?;

    let mut socket = UdpSocket::bind(local_addr).await?;
    const MAX_DATAGRAM_SIZE: usize = 65_507;
    socket.connect(&remote_addr).await?;
    {
        let span = debug_span!("client send");
        let _g = span.enter();
        span_context!(span, Level::DEBUG);
        let data = span.get_context_bytes();

        socket.send(data.as_ref()).await?;
    }
    {
        let mut data = vec![0u8; MAX_DATAGRAM_SIZE];
        let len = socket.recv(&mut data).await?;
        let data = data[..len].to_vec();
        let span = debug_span!("client recv");
        let _g = span.enter();
        span.set_from_bytes(data);
        span_context!(span, Level::DEBUG);

        let data = span.get_context_bytes();

        socket.send(data.as_ref()).await?;
    }
    {
        let mut data = vec![0u8; MAX_DATAGRAM_SIZE];
        let len = socket.recv(&mut data).await?;
        let data = data[..len].to_vec();
        let span = debug_span!("client recv 2");
        let _g = span.enter();
        span.set_from_bytes(data);
        span_context!(span, Level::DEBUG);
    }
    Ok(())
}
*/
