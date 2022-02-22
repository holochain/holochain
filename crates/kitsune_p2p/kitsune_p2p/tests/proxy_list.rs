use kitsune_p2p_types::config::*;
use kitsune_p2p_types::tls::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_transport_quic::tx2::*;
use futures::stream::StreamExt;

#[tokio::test(flavor = "multi_thread")]
async fn test_integrated_proxy_list() {
    let tuning_params = KitsuneP2pTuningParams::default();

    // -- set up proxy -- //

    let p_tls = TlsConfig::new_ephemeral().await.unwrap();
    let mut conf = QuicConfig::default();
    conf.tls = Some(p_tls.clone());
    conf.tuning_params = Some(tuning_params.clone());

    let f = QuicBackendAdapt::new(conf).await.unwrap();
    let f = tx2_pool_promote(f, Default::default());
    let mut conf = ProxyConfig::default();
    conf.tuning_params = Some(tuning_params.clone());
    conf.allow_proxy_fwd = true;
    let f = tx2_proxy(f, conf).unwrap();
    let mut proxy = f
        .bind(
            "kitsune-quic://127.0.0.1:0".into(),
            tuning_params.implicit_timeout(),
        )
        .await
        .unwrap();

    let hnd = proxy.handle().clone();
    let proxy_url = hnd.local_addr().unwrap();

    tokio::task::spawn(async move {
        while proxy.next().await.is_some() {}
    });

    println!("proxy_url: {}", proxy_url);

    // -- set up bootstrap -- //

    let (driver, bootstrap_url) = kitsune_p2p_bootstrap::run(
        ([127, 0, 0, 1], 0),
        vec![proxy_url.as_str().into()],
    ).await.unwrap();
    let bootstrap_url = format!("http://{}", bootstrap_url);

    tokio::task::spawn(driver);

    println!("bootstrap_url: {}", bootstrap_url);

    // -- shutdown -- //

    // TODO: shutdown bootstrap server
    hnd.close(0, "").await;
}
