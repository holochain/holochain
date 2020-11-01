use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_proxy::*;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::{
    dependencies::{ghost_actor, serde_json},
    transport::*,
};
use structopt::StructOpt;

mod opt;
use opt::*;

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

    let (listener, events) = spawn_transport_listener_quic(opt.into()).await?;

    let proxy_config = ProxyConfig::local_proxy_server(
        TlsConfig::new_ephemeral().await?,
        AcceptProxyCallback::accept_all(),
    );

    let (listener, mut events) =
        spawn_kitsune_proxy_listener(proxy_config, listener, events).await?;

    println!("{}", listener.bound_url().await?);

    tokio::task::spawn(async move {
        while let Some(evt) = events.next().await {
            match evt {
                TransportEvent::IncomingChannel(url, mut write, _read) => {
                    tracing::debug!(
                        "{} is trying to talk directly to us - dump proxy state",
                        url
                    );
                    match listener.debug().await {
                        Ok(dump) => {
                            let dump = serde_json::to_string_pretty(&dump).unwrap();
                            let _ = write.write_and_close(dump.into_bytes()).await;
                        }
                        Err(e) => {
                            let _ = write.write_and_close(format!("{:?}", e).into_bytes()).await;
                        }
                    }
                }
            }
        }
    });

    // wait for ctrl-c
    futures::future::pending().await
}
