use futures::stream::StreamExt;
use kitsune_p2p::actor::*;
use kitsune_p2p::*;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_proxy::*;
use kitsune_p2p_transport_quic::tx2::*;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::tls::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;

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
    let mut conf = kitsune_p2p_proxy::tx2::ProxyConfig::default();
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

    tokio::task::spawn(async move { while proxy.next().await.is_some() {} });

    println!("proxy_url: {}", proxy_url);

    // -- set up bootstrap -- //

    let (driver, bootstrap_url) =
        kitsune_p2p_bootstrap::run(([127, 0, 0, 1], 0), vec![proxy_url.as_str().into()])
            .await
            .unwrap();
    let bootstrap_url = url2::Url2::parse(format!("http://{}", bootstrap_url));

    tokio::task::spawn(driver);

    println!("bootstrap_url: {}", bootstrap_url);

    // -- start kitsune -- //

    let k_tls = TlsConfig::new_ephemeral().await.unwrap();
    let mut kconf = KitsuneP2pConfig::default();
    kconf.tuning_params = tuning_params.clone();
    kconf.bootstrap_service = Some(bootstrap_url.clone());
    kconf.transport_pool = vec![TransportConfig::Proxy {
        sub_transport: Box::new(TransportConfig::Quic {
            bind_to: Some(url2::Url2::parse("kitsune-quic://127.0.0.1:0")),
            override_host: None,
            override_port: None,
        }),
        proxy_config: kitsune_p2p::ProxyConfig::RemoteProxyClientFromBootstrap {
            bootstrap_url,
            fallback_proxy_url: None,
        },
    }];

    let (actor, mut evt) = spawn_kitsune_p2p(kconf, k_tls, HostStub::new())
        .await
        .unwrap();

    tokio::task::spawn(async move {
        while let Some(e) = evt.next().await {
            println!("{:?}", e);
        }
    });

    let url = actor.list_transport_bindings().await.unwrap().remove(0);
    println!("Bound to: {}", url);

    // make sure this is not just a clone of the proxy address
    assert_ne!(
        ProxyUrl::from(proxy_url.as_str()).digest(),
        ProxyUrl::from(url.as_str()).digest()
    );
    // make sure it *is* pointing at the proxy's port
    assert_eq!(proxy_url.port(), url.port());

    // -- shutdown -- //

    use kitsune_p2p_types::dependencies::ghost_actor::GhostControlSender;
    actor.ghost_actor_shutdown_immediate().await.unwrap();
    // TODO: shutdown bootstrap server
    hnd.close(0, "").await;
}
