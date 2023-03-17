fn main() {}
/*
use holochain_trace::{span_context, OpenSpanExt};
use std::{env, error::Error};
use tokio::net::UdpSocket;
use tracing::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    holochain_trace::test_run_open().ok();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let mut socket = UdpSocket::bind(&addr).await?;
    println!("Listening on: {}", socket.local_addr()?);

    {
        let mut buf = vec![0; 1024];
        let (size, peer) = socket.recv_from(&mut buf).await?;

        let data = buf[..size].to_vec();
        let span = debug_span!("server recv");
        span.set_from_bytes(data);
        let _g = span.enter();
        span_context!(span, Level::DEBUG);
        let span = debug_span!("inner 1");
        let _g = span.enter();
        span_context!(span, Level::DEBUG);
        let span = debug_span!("inner 2");
        let _g = span.enter();
        span_context!(span, Level::DEBUG);
        let data = span.get_context_bytes();
        let _amt = socket.send_to(&data[..], &peer).await?;
    }

    {
        let mut buf = vec![0; 1024];
        let (size, peer) = socket.recv_from(&mut buf).await?;

        let data = buf[..size].to_vec();
        let span = debug_span!("server recv 2");
        span.set_from_bytes(data);
        let _g = span.enter();
        span_context!(span, Level::DEBUG);
        let data = span.get_context_bytes();
        let _amt = socket.send_to(&data[..], &peer).await?;
    }
    Ok(())
}
*/
