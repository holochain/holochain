use futures::stream::StreamExt;
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

#[tokio::test(flavor = "multi_thread")]
async fn test_proxy_integration() {
    if let Err(e) = test_inner().await {
        panic!("{:?}", e);
    }
}

async fn connect(
    proxy_config: Arc<ProxyConfig>,
) -> TransportResult<ghost_actor::GhostSender<TransportListener>> {
    let (bind, evt) = kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?;
    let addr = bind.bound_url().await?;
    tracing::warn!("got bind: {}", addr);

    let (bind, mut evt) = spawn_kitsune_proxy_listener(
        proxy_config,
        Arc::new(KitsuneP2pTuningParams::default()),
        bind,
        evt,
    )
    .await?;
    let addr = bind.bound_url().await?;
    tracing::warn!("got proxy: {}", addr);

    tokio::task::spawn(async move {
        while let Some(evt) = evt.next().await {
            match evt {
                TransportEvent::IncomingChannel(url, mut write, read) => {
                    tracing::warn!("Incoming PROXY: {}", url);
                    let data = read.read_to_end().await;
                    let data = String::from_utf8_lossy(&data);
                    tracing::warn!("PROXY_READ_DATA: {}", data);
                    let data = format!("echo: {}", data);
                    write.write_and_close(data.into_bytes()).await?;
                }
            }
        }
        TransportResult::Ok(())
    });

    Ok(bind)
}

async fn test_inner() -> TransportResult<()> {
    init_tracing();

    let proxy_config1 = ProxyConfig::local_proxy_server(
        TlsConfig::new_ephemeral().await?,
        AcceptProxyCallback::accept_all(),
    );
    let bind1 = connect(proxy_config1).await?;
    let addr1 = bind1.bound_url().await?;

    let proxy_config2 = ProxyConfig::local_proxy_server(
        TlsConfig::new_ephemeral().await?,
        AcceptProxyCallback::accept_all(),
    );
    let bind2 = connect(proxy_config2).await?;

    let proxy_config3 =
        ProxyConfig::remote_proxy_client(TlsConfig::new_ephemeral().await?, addr1.into());
    let bind3 = connect(proxy_config3).await?;
    let addr3 = bind3.bound_url().await?;

    let (_url, mut write, read) = bind2.create_channel(addr3.clone()).await?;
    write.write_and_close(b"test".to_vec()).await?;
    let data = read.read_to_end().await;
    let data = String::from_utf8_lossy(&data);
    assert_eq!("echo: test", data);

    // run a second time to prove out session resumption
    let (_url, mut write, read) = bind2.create_channel(addr3).await?;
    write.write_and_close(b"test".to_vec()).await?;
    let data = read.read_to_end().await;
    let data = String::from_utf8_lossy(&data);
    assert_eq!("echo: test", data);

    tracing::warn!("TEST COMPLETE");

    Ok(())
}
