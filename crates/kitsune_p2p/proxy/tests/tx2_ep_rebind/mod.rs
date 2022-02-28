use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_proxy::ProxyUrl;
use kitsune_p2p_transport_quic::tx2::*;
use kitsune_p2p_types::dependencies::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::tx2::tx2_restart_adapter::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::*;

kitsune_p2p_types::write_codec_enum! {
    codec Wire {
        Req(0x01) {
            data.0: PoolBuf,
        },
        Res(0x02) {
            data.0: PoolBuf,
        },
    }
}

async fn gen_node(maybe_proxy: Option<ProxyUrl>) -> Tx2EpHnd<Wire> {
    let conf = QuicConfig::default();
    let f = tx2_quic_adapter(conf).await.unwrap();
    let f = tx2_restart_adapter(f);
    let f = tx2_pool_promote(f, Default::default());
    let mut conf = ProxyConfig::default();
    conf.allow_proxy_fwd = true;
    conf.client_of_remote_proxy = match maybe_proxy {
        None => ProxyRemoteType::NoProxy,
        Some(proxy_url) => ProxyRemoteType::Specific(proxy_url.as_str().into()),
    };
    let f = tx2_proxy(f, conf).unwrap();
    let f = tx2_api::<Wire>(f, Default::default());

    let ep = f
        .bind(
            "kitsune-quic://0.0.0.0:0",
            KitsuneTimeout::from_millis(5000),
        )
        .await
        .unwrap();

    let ep_hnd = ep.handle().clone();

    tokio::task::spawn(async move {
        ep.for_each_concurrent(10, move |evt| async move {
            use Tx2EpEvent::*;
            match evt {
                IncomingRequest(Tx2EpIncomingRequest { data, respond, .. }) => {
                    if let Wire::Req(Req { mut data }) = data {
                        assert_eq!(b"hello", data.as_ref());
                        data.clear();
                        data.extend_from_slice(b"world");
                        let _ = respond
                            .respond(Wire::res(data), KitsuneTimeout::from_millis(5000))
                            .await;
                    } else {
                        panic!("unexpected: {:?}", data);
                    }
                }
                IncomingConnection(_) | OutgoingConnection(_) | Tick => (),
                EndpointClosed => {
                    println!("got endpoint closed... let's see if we can still communicate : )");
                }
                evt => println!("node handler unhandled: {:?}", evt),
            }
        })
        .await;
    });

    ep_hnd
}

fn init_tracing() {
    let _ = tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn tx2_ep_rebind() {
    init_tracing();

    let t = KitsuneTimeout::from_millis(5000);

    let proxy_hnd = gen_node(None).await;
    let proxy_addr = proxy_hnd.local_addr().unwrap();
    println!("proxy: {}", proxy_addr);

    let tgt_hnd = gen_node(Some(ProxyUrl::from(proxy_addr.as_str()))).await;
    let _ = tgt_hnd.get_connection(proxy_addr.clone(), t).await.unwrap();
    let tgt_addr = tgt_hnd.local_addr().unwrap();
    println!("tgt: {}", tgt_addr);

    let node = gen_node(Some(ProxyUrl::from(proxy_addr.as_str()))).await;

    let node_addr = node.local_addr().unwrap();
    println!("@@@ node @@@: {}", node_addr);

    //tracing::error!("-- test -- closing node");

    // shut down the whole endpoint.
    // this is simulating the endpoint shutting down,
    // e.g. iface down / cable unplugged / airplane mode.
    node.close(999, "noodle").await;

    //tracing::error!("-- test -- sleeping");

    // give the node some time to re-connect
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let node_addr = node.local_addr().unwrap();
    println!("@@@ node @@@: {}", node_addr);

    //tracing::error!("-- test -- making request");

    // make sure we can receive requests from someone else
    let mut data = PoolBuf::new();
    data.extend_from_slice(b"hello");
    let res = tgt_hnd
        .request(node_addr.clone(), &Wire::req(data), t)
        .await
        .unwrap();
    if let Wire::Res(Res { data }) = res {
        assert_eq!(b"world", data.as_ref());
    } else {
        panic!("unexpected: {:?}", res);
    }

    // make sure we can make outgoing requests
    let mut data = PoolBuf::new();
    data.extend_from_slice(b"hello");
    let res = node
        .request(tgt_addr.clone(), &Wire::req(data), t)
        .await
        .unwrap();
    if let Wire::Res(Res { data }) = res {
        assert_eq!(b"world", data.as_ref());
    } else {
        panic!("unexpected: {:?}", res);
    }
}
