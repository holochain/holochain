use futures::stream::StreamExt;
use kitsune_p2p_proxy::*;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::metrics::metric_task;
use kitsune_p2p_types::transport::*;
use std::sync::Arc;
use structopt::StructOpt;

/// Option Parsing
#[derive(structopt::StructOpt, Debug)]
#[structopt(name = "proxy-cli")]
pub struct Opt {
    /// kitsune-proxy Url to connect to.
    pub proxy_url: String,

    /// If you would like to keep pinging, set an interval here.
    #[structopt(short = "t", long)]
    pub time_interval_ms: Option<u64>,
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

    let tuning_params = Arc::new(KitsuneP2pTuningParams::default());

    let (listener, events) = spawn_transport_listener_quic(ConfigListenerQuic::default()).await?;

    let proxy_config = ProxyConfig::local_proxy_server(
        TlsConfig::new_ephemeral().await?,
        AcceptProxyCallback::reject_all(),
    );

    let (listener, mut events) =
        spawn_kitsune_proxy_listener(proxy_config, tuning_params, listener, events).await?;

    metric_task(async move {
        while let Some(evt) = events.next().await {
            match evt {
                TransportEvent::IncomingChannel(url, mut write, read) => {
                    eprintln!("# ERR incoming msg from {}", url);
                    drop(read);
                    let _ = write.write_and_close(Vec::with_capacity(0)).await;
                }
            }
        }
        <Result<(), ()>>::Ok(())
    });

    let proxy_url = ProxyUrl::from(&opt.proxy_url);

    loop {
        println!("# Attempting to connect to {}", proxy_url);

        let (_url, mut write, read) = listener.create_channel((&proxy_url).into()).await?;
        write.write_and_close(Vec::with_capacity(0)).await?;
        let res = read.read_to_end().await;
        println!(
            "#DEBUG:START#\n{}\n#DEBUG:END#",
            String::from_utf8_lossy(&res)
        );

        match &opt.time_interval_ms {
            None => break,
            Some(ms) => {
                tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
            }
        }
    }

    Ok(())
}
