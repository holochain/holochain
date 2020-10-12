use futures::stream::StreamExt;
use kitsune_p2p_proxy::*;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::{dependencies::ghost_actor, transport::*};
use structopt::StructOpt;

/// Option Parsing
#[derive(structopt::StructOpt, Debug)]
#[structopt(name = "proxy-cli")]
pub struct Opt {
    /// kitsune-proxy Url to connect to.
    pub proxy_url: String,
}

#[tokio::main]
async fn main() {
    let _ = ghost_actor::dependencies::tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    );

    if let Err(e) = inner().await {
        eprintln!("{:?}", e);
    }
}

async fn inner() -> TransportResult<()> {
    let opt = Opt::from_args();

    let (listener, events) = spawn_transport_listener_quic(ConfigListenerQuic::default()).await?;

    let proxy_config = ProxyConfig::local_proxy_server(
        TlsConfig::new_ephemeral().await?,
        AcceptProxyCallback::reject_all(),
    );

    let (listener, mut events) =
        spawn_kitsune_proxy_listener(proxy_config, listener, events).await?;

    tokio::task::spawn(async move {
        while let Some((url, mut write, read)) = events.next().await {
            eprintln!("ERR incoming msg from {}", url);
            drop(read);
            let _ = write.write_and_close(Vec::with_capacity(0)).await;
        }
    });

    println!(
        "SELF URL (you can ignore this : ) {}",
        listener.bound_url().await?
    );

    let proxy_url = ProxyUrl::from(&opt.proxy_url);
    println!("Attempting to connect to {}", proxy_url);

    let (_url, mut write, read) = listener.create_channel(proxy_url.into()).await?;
    write.write_and_close(b"test".to_vec()).await?;
    let res = read.read_to_end().await;
    println!("{}", String::from_utf8_lossy(&res));

    Ok(())
}
