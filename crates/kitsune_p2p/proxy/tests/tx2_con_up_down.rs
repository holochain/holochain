use futures::stream::StreamExt;
use kitsune_p2p_proxy::tx2::*;
use kitsune_p2p_proxy::ProxyUrl;
use kitsune_p2p_transport_quic::tx2::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
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

async fn gen_node() -> Tx2EpHnd<Wire> {
    let conf = QuicConfig::default();
    let f = tx2_quic_adapter(conf).await.unwrap();
    let f = tx2_pool_promote(f, Default::default());
    let mut conf = ProxyConfig::default();
    conf.allow_proxy_fwd = true;
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
                evt => println!("unhandled: {:?}", evt),
            }
        })
        .await;
    });

    ep_hnd
}

fn proxify_addr(purl: &TxUrl, nurl: &TxUrl) -> TxUrl {
    let digest = ProxyUrl::from(nurl.as_str());
    let digest = digest.digest();
    let purl = ProxyUrl::from(purl.as_str());
    ProxyUrl::new(purl.as_base().as_str(), digest)
        .unwrap()
        .as_str()
        .into()
}

#[tokio::test(flavor = "multi_thread")]
async fn tx2_con_up_down_test() {
    let t = KitsuneTimeout::from_millis(5000);

    let proxy_hnd = gen_node().await;
    let proxy_addr = proxy_hnd.local_addr().unwrap();
    println!("proxy: {}", proxy_addr);

    let tgt_hnd = gen_node().await;
    let _ = tgt_hnd.get_connection(proxy_addr.clone(), t).await.unwrap();
    let tgt_addr = tgt_hnd.local_addr().unwrap();
    let tgt_addr = proxify_addr(&proxy_addr, &tgt_addr);
    println!("tgt: {}", tgt_addr);

    let node = gen_node().await;

    let test = || async {
        // give the closes some time to effect.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

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
    };

    // make sure the basic request works
    test().await;

    // close the node-to-proxy con, and re-test the request
    node.close_connection(proxy_addr.clone(), 999, "noodle1")
        .await;
    test().await;

    // close/reopen the tgt-to-proxy con, and re-test the request
    tgt_hnd
        .close_connection(proxy_addr.clone(), 999, "noodle2")
        .await;
    let _ = tgt_hnd.get_connection(proxy_addr.clone(), t).await.unwrap();
    test().await;
}
