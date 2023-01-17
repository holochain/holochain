use futures::stream::StreamExt;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_transport_quic::tx2::*;
use kitsune_p2p_types::metrics::*;
use kitsune_p2p_types::tls::*;
use kitsune_p2p_types::tx2::tx2_pool::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::*;
use structopt::StructOpt;

/// Option Parsing
#[derive(structopt::StructOpt, Debug)]
#[structopt(name = "proxy-tx2-cli")]
pub struct Opt {
    /// kitsune-proxy Url to connect to.
    pub proxy_url: String,
}

#[tokio::main]
async fn main() {
    observability::test_run().ok();

    if let Err(e) = inner().await {
        eprintln!("{:?}", e);
    }
}

async fn inner() -> KitsuneResult<()> {
    let opt = Opt::from_args();

    let tls_config = TlsConfig::new_ephemeral().await?;
    let mut conf = QuicConfig::default();
    conf.tls = Some(tls_config.clone());
    let f = QuicBackendAdapt::new(conf).await?;
    let f = tx2_pool_promote(f, Default::default());
    let f = tx2_proxy(f, Default::default())?;

    let t = KitsuneTimeout::from_millis(60 * 1000);

    let mut ep = f.bind("kitsune-quic://0.0.0.0:0".into(), t).await?;

    let ep_hnd = ep.handle().clone();

    let task = metric_task(async move {
        while let Some(evt) = ep.next().await {
            if let EpEvent::IncomingData(EpIncomingData { data, .. }) = evt {
                println!("{}", String::from_utf8_lossy(data.as_ref()));
                break;
            }
        }
        KitsuneResult::Ok(())
    });

    let con = ep_hnd.get_connection(opt.proxy_url.into(), t).await?;

    con.write(0.into(), PoolBuf::new(), t).await?;

    task.await.map_err(KitsuneError::other)??;

    ep_hnd.close(0, "").await;

    Ok(())
}
