use super::*;
use futures::stream::StreamExt;
use kitsune_p2p_direct::dependencies::*;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_transport_quic::tx2::*;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::tls::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;

pub(crate) async fn run(_opt: KdOptProxy) -> KitsuneResult<()> {
    let tuning_params = KitsuneP2pTuningParams::default();

    let p_tls = TlsConfig::new_ephemeral().await?;
    let mut conf = QuicConfig::default();
    conf.tls = Some(p_tls.clone());
    conf.tuning_params = Some(tuning_params.clone());

    let f = QuicBackendAdapt::new(conf).await?;
    let f = tx2_pool_promote(f, tuning_params.clone());
    let mut conf = ProxyConfig::default();
    conf.tuning_params = Some(tuning_params.clone());
    conf.allow_proxy_fwd = true;
    let f = tx2_proxy(f, conf)?;

    let mut proxy = f
        .bind(
            "kitsune-quic://0.0.0.0:0".into(),
            tuning_params.implicit_timeout(),
        )
        .await?;

    let proxy_hnd = proxy.handle().clone();
    let proxy_url = proxy_hnd.local_addr()?;
    println!("{}", proxy_url);

    while proxy.next().await.is_some() {}

    Ok(())
}
