//use kitsune_p2p_proxy::*;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::transport::*;

#[tokio::main]
async fn main() {
    if let Err(e) = inner().await {
        eprintln!("{:?}", e);
    }
}

async fn inner() -> TransportResult<()> {
    let config = ConfigListenerQuic::default();
    let (listener, _events) = spawn_transport_listener_quic(config).await?;
    println!("Hello: {}", listener.bound_url().await?);
    Ok(())
}
