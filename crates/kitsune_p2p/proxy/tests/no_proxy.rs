use ghost_actor::dependencies::tracing;
use kitsune_p2p_proxy::*;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::transport::*;
use std::sync::Arc;

fn init_tracing() {
    let _ = ghost_actor::dependencies::tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    );
}

#[tokio::test(threaded_scheduler)]
async fn test_no_proxy() {
    if let Err(e) = test_inner().await {
        panic!("{:?}", e);
    }
}

async fn test_inner() -> TransportResult<()> {
    init_tracing();

    const FAKE_ADDR: &'static str = "kitsune-proxy://FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAK/kitsune-mem/h/FAKEFAKEFAKEFAKEFAKEF/--";

    let proxy_config =
        ProxyConfig::remote_proxy_client(TlsConfig::new_ephemeral().await?, FAKE_ADDR.into());

    let (bind, evt) = kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?;
    let (_bind, _evt) = spawn_kitsune_proxy_listener(
        proxy_config,
        Arc::new(KitsuneP2pTuningParams::default()),
        bind,
        evt,
    )
    .await?;

    tracing::warn!("TEST COMPLETE");

    Ok(())
}
