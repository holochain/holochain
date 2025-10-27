use crate::tests::common::{spawn_test_bootstrap, Handler};
use holochain_keystore::*;
use holochain_p2p::event::*;
use holochain_p2p::*;
use holochain_trace::test_run;
use holochain_types::prelude::*;
use kitsune2_api::*;
use std::net::SocketAddr;
use std::{sync::Arc, time::Duration};

const UNRESPONSIVE_TIMEOUT: Duration = Duration::from_secs(15);
const WAIT_BETWEEN_CALLS: Duration = Duration::from_millis(10);

/// An implementation of [`HcP2pHandler`] that doesn't ever respond to requests
#[derive(Clone, Debug)]
struct UnresponsiveHandler;

impl HcP2pHandler for UnresponsiveHandler {
    fn handle_call_remote(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _zome_call_params_serialized: ExternIO,
        _signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>> {
        Box::pin(std::future::pending())
    }

    fn handle_publish(
        &self,
        _dna_hash: DnaHash,
        _ops: Vec<holochain_types::dht_op::DhtOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(std::future::pending())
    }

    fn handle_get(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _dht_hash: holo_hash::AnyDhtHash,
    ) -> BoxFut<'_, HolochainP2pResult<WireOps>> {
        Box::pin(std::future::pending())
    }

    fn handle_get_links(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _link_key: WireLinkKey,
        _options: GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireLinkOps>> {
        Box::pin(std::future::pending())
    }

    fn handle_count_links(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        Box::pin(std::future::pending())
    }

    fn handle_get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _agent: AgentPubKey,
        _query: ChainQueryFilter,
        _options: GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<AgentActivityResponse>> {
        Box::pin(std::future::pending())
    }

    fn handle_must_get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _author: AgentPubKey,
        _filter: holochain_zome_types::chain::ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<MustGetAgentActivityResponse>> {
        Box::pin(std::future::pending())
    }

    fn handle_validation_receipts_received(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(std::future::pending())
    }

    fn handle_publish_countersign(
        &self,
        _dna_hash: DnaHash,
        _op: holochain_types::dht_op::ChainOp,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(std::future::pending())
    }

    fn handle_countersigning_session_negotiation(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _message: CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(std::future::pending())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_call_remote() {
    test_run();

    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (agent2, hc2, _) = spawn_test(dna_hash.clone(), handler, &addr).await;

    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // make sure hc2 has its own address
            #[allow(clippy::len_zero)] // !<7 lines>.is_empty() is NOT clearer!
            if hc2
                .peer_store(dna_hash.clone())
                .await
                .unwrap()
                .get_all()
                .await
                .unwrap()
                .len()
                > 0
            {
                break;
            }
        }

        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // Make sure the hc2 peer store has agent1's address
            if hc2
                .peer_store(dna_hash.clone())
                .await
                .unwrap()
                .get(agent1.to_k2_agent())
                .await
                .unwrap()
                .is_some()
            {
                break;
            }
        }

        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            let resp = hc2
                .call_remote(
                    dna_hash.clone(),
                    agent1.clone(),
                    ExternIO(b"hello".to_vec()),
                    Signature([0; 64]),
                )
                .await
                .unwrap();
            let resp: Vec<u8> = UnsafeBytes::from(resp).into();
            let resp = String::from_utf8_lossy(&resp);
            assert_eq!("got_call_remote: hello", resp);

            // break when hc1 has hc2's address
            if hc1
                .peer_store(dna_hash.clone())
                .await
                .unwrap()
                .get_all()
                .await
                .unwrap()
                .len()
                > 1
            {
                break;
            }
        }
    })
    .await
    .unwrap();

    let resp = hc1
        .call_remote(
            dna_hash,
            agent2,
            ExternIO(b"world".to_vec()),
            Signature([0; 64]),
        )
        .await
        .unwrap();
    let resp: Vec<u8> = UnsafeBytes::from(resp).into();
    let resp = String::from_utf8_lossy(&resp);
    assert_eq!("got_call_remote: world", resp);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_signal() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (agent1, _hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    hc2.send_remote_signal(
        dna_hash,
        vec![(agent1, ExternIO(b"hello".to_vec()), Signature([0; 64]))],
    )
    .await
    .unwrap();

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            if let Some(res) = handler.calls.lock().unwrap().first() {
                assert_eq!("got_call_remote: hello", res);
                break;
            }
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;
        }
    })
    .await
    .unwrap();
}

fn test_dht_op(authored_timestamp: holochain_types::prelude::Timestamp) -> DhtOpHashed {
    let mut create = ::fixt::fixt!(Create);
    create.timestamp = authored_timestamp;

    let op = DhtOp::from(ChainOp::StoreRecord(
        ::fixt::fixt!(Signature),
        Action::Create(create),
        RecordEntry::Present(::fixt::fixt!(Entry)),
    ));
    DhtOpHashed::from_content_sync(op)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_publish() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    let op = test_dht_op(holochain_types::prelude::Timestamp::now());
    let op_hash = op.as_hash().clone();

    // TODO invoking process_incoming_ops is a hack,
    //      prefer calling a function on the mem store directly.
    hc2.test_kitsune()
        .space(dna_hash.to_k2_space())
        .await
        .unwrap()
        .op_store()
        .process_incoming_ops(vec![bytes::Bytes::from(
            holochain_serialized_bytes::encode(op.as_content()).unwrap(),
        )])
        .await
        .unwrap();

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            hc2.publish(
                dna_hash.clone(),
                HoloHash::from_raw_36_and_type(
                    op_hash.get_raw_36().to_vec(),
                    holo_hash::hash_type::AnyLinkable::Action,
                ),
                AgentPubKey::from_raw_32(vec![2; 32]),
                vec![op_hash.clone()],
                None,
                None,
            )
            .await
            .unwrap();

            if let Some(res) = handler.calls.lock().unwrap().first() {
                assert_eq!("publish", res);
                break;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_publish_reflect() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            let op = test_dht_op(holochain_types::prelude::Timestamp::now());
            let op_hash = op.as_hash();

            hc2.publish(
                dna_hash.clone(),
                HoloHash::from_raw_36_and_type(
                    op_hash.get_raw_36().to_vec(),
                    holo_hash::hash_type::AnyLinkable::Action,
                ),
                AgentPubKey::from_raw_32(vec![2; 32]),
                vec![],
                None,
                Some(vec![op.into_content()]),
            )
            .await
            .unwrap();

            if let Some(res) = handler.calls.lock().unwrap().first() {
                assert_eq!("publish", res);
                break;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler, &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // if we get a response at all, the full back-n-forth succeeded
            if hc2
                .get(
                    dna_hash.clone(),
                    HoloHash::from_raw_36_and_type(
                        vec![1; 36],
                        holo_hash::hash_type::AnyDht::Entry,
                    ),
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_with_unresponsive_agents() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::new(
        WireOps::Record(WireRecordOps {
            entry: Some(Entry::Agent(fake_agent_pubkey_1())),
            ..Default::default()
        }),
        Some(Duration::from_millis(500)),
    ));
    let unresponsive_handler = Arc::new(UnresponsiveHandler);

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), unresponsive_handler.clone(), &addr).await;
    let (_agent3, hc3, _) = spawn_test(dna_hash.clone(), unresponsive_handler, &addr).await;
    let (_agent4, hc4, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;
    hc3.test_set_full_arcs(space.clone()).await;
    hc4.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // If we get a response at all then at least one peer completed the request
            if hc1
                .get(
                    dna_hash.clone(),
                    HoloHash::from_raw_36_and_type(
                        vec![1; 36],
                        holo_hash::hash_type::AnyDht::Entry,
                    ),
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    })
    .await
    .unwrap();

    let requests = handler.calls.lock().unwrap();
    assert_eq!(*requests, ["get"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_when_not_all_agents_have_data() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let wire_ops = WireOps::Record(WireRecordOps {
        entry: Some(Entry::Agent(fake_agent_pubkey_1())),
        ..Default::default()
    });
    let handler = Arc::new(Handler::new(
        wire_ops.clone(),
        Some(Duration::from_millis(500)),
    ));
    let empty_handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), empty_handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), empty_handler.clone(), &addr).await;
    let (_agent3, hc3, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;
    hc3.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // Wait until we get the response we want
            if let Ok(response) = hc1
                .get(
                    dna_hash.clone(),
                    HoloHash::from_raw_36_and_type(
                        vec![1; 36],
                        holo_hash::hash_type::AnyDht::Entry,
                    ),
                )
                .await
            {
                if response.first().unwrap() == &wire_ops {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();

    let requests = empty_handler.calls.lock().unwrap();
    if !requests.is_empty() {
        assert!(
            requests.iter().all(|r| r == "get"),
            "All requests to empty handler should be 'get'"
        );
    }

    let requests = handler.calls.lock().unwrap();
    assert_eq!(*requests, ["get"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_when_not_all_agents_have_data_and_unresponsive_agent() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let wire_ops = WireOps::Record(WireRecordOps {
        entry: Some(Entry::Agent(fake_agent_pubkey_1())),
        ..Default::default()
    });
    let handler = Arc::new(Handler::new(
        wire_ops.clone(),
        Some(Duration::from_millis(500)),
    ));
    let empty_handler = Arc::new(Handler::default());
    let unresponsive_handler = Arc::new(UnresponsiveHandler);

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), empty_handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), empty_handler.clone(), &addr).await;
    let (_agent3, hc3, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent4, hc4, _) = spawn_test(dna_hash.clone(), unresponsive_handler.clone(), &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;
    hc3.test_set_full_arcs(space.clone()).await;
    hc4.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // Wait until we get the response we want
            if let Ok(response) = hc1
                .get(
                    dna_hash.clone(),
                    HoloHash::from_raw_36_and_type(
                        vec![1; 36],
                        holo_hash::hash_type::AnyDht::Entry,
                    ),
                )
                .await
            {
                if response.first().unwrap() == &wire_ops {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();

    let requests = empty_handler.calls.lock().unwrap();
    if !requests.is_empty() {
        assert!(
            requests.iter().all(|r| r == "get"),
            "All requests to empty handler should be 'get'"
        );
    }

    let requests = handler.calls.lock().unwrap();
    assert_eq!(*requests, ["get"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_empty_data_better_than_no_response() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let empty_handler = Arc::new(Handler::default());
    let unresponsive_handler = Arc::new(UnresponsiveHandler);

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), empty_handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), empty_handler.clone(), &addr).await;
    let (_agent3, hc3, _) = spawn_test(dna_hash.clone(), unresponsive_handler.clone(), &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;
    hc3.test_set_full_arcs(space.clone()).await;

    // One agent will respond with empty data so we need to wait for the other one to timeout
    // before we will get the empty data.
    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            if hc1
                .get(
                    dna_hash.clone(),
                    HoloHash::from_raw_36_and_type(
                        vec![1; 36],
                        holo_hash::hash_type::AnyDht::Entry,
                    ),
                )
                .await
                .is_ok()
            {
                break;
            }
        }
    })
    .await
    .unwrap();

    let requests = empty_handler.calls.lock().unwrap();
    assert_eq!(*requests, ["get"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_links() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler, &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // if we get a response at all, the full back-n-forth succeeded
            if hc2
                .get_links(
                    dna_hash.clone(),
                    WireLinkKey {
                        base: HoloHash::from_raw_36_and_type(
                            vec![1; 36],
                            holo_hash::hash_type::AnyDht::Entry,
                        )
                        .into(),
                        type_query: LinkTypeFilter::Types(Vec::new()),
                        tag: None,
                        after: None,
                        before: None,
                        author: None,
                    },
                    holochain_p2p::actor::GetLinksOptions::default(),
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_links_with_unresponsive_agents() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());
    let unresponsive_handler = Arc::new(UnresponsiveHandler);

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent3, hc3, _) = spawn_test(dna_hash.clone(), unresponsive_handler.clone(), &addr).await;
    let (_agent4, hc4, _) = spawn_test(dna_hash.clone(), unresponsive_handler, &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;
    hc3.test_set_full_arcs(space.clone()).await;
    hc4.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // If we get a response at all then at least one peer completed the request
            if hc1
                .get_links(
                    dna_hash.clone(),
                    WireLinkKey {
                        base: HoloHash::from_raw_36_and_type(
                            vec![1; 36],
                            holo_hash::hash_type::AnyDht::Entry,
                        )
                        .into(),
                        type_query: LinkTypeFilter::Types(Vec::new()),
                        tag: None,
                        after: None,
                        before: None,
                        author: None,
                    },
                    holochain_p2p::actor::GetLinksOptions::default(),
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    })
    .await
    .unwrap();

    let requests = handler.calls.lock().unwrap();
    assert_eq!(*requests, ["get_links"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_count_links() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler, &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // if we get a response at all, the full back-n-forth succeeded
            if hc2
                .count_links(
                    dna_hash.clone(),
                    WireLinkQuery {
                        base: HoloHash::from_raw_36_and_type(
                            vec![1; 36],
                            holo_hash::hash_type::AnyDht::Entry,
                        )
                        .into(),
                        link_type: LinkTypeFilter::Types(Vec::new()),
                        tag_prefix: None,
                        before: None,
                        after: None,
                        author: None,
                    },
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_count_links_with_unresponsive_agents() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());
    let unresponsive_handler = Arc::new(UnresponsiveHandler);

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent3, hc3, _) = spawn_test(dna_hash.clone(), unresponsive_handler.clone(), &addr).await;
    let (_agent4, hc4, _) = spawn_test(dna_hash.clone(), unresponsive_handler, &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;
    hc3.test_set_full_arcs(space.clone()).await;
    hc4.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // If we get a response at all then at least one peer completed the request
            if hc1
                .count_links(
                    dna_hash.clone(),
                    WireLinkQuery {
                        base: HoloHash::from_raw_36_and_type(
                            vec![1; 36],
                            holo_hash::hash_type::AnyDht::Entry,
                        )
                        .into(),
                        link_type: LinkTypeFilter::Types(Vec::new()),
                        tag_prefix: None,
                        before: None,
                        after: None,
                        author: None,
                    },
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    })
    .await
    .unwrap();

    let requests = handler.calls.lock().unwrap();
    assert_eq!(*requests, ["count_links"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_agent_activity() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler, &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // if we get a response at all, the full back-n-forth succeeded
            if hc2
                .get_agent_activity(
                    dna_hash.clone(),
                    AgentPubKey::from_raw_36(vec![2; 36]),
                    ChainQueryFilter {
                        sequence_range: ChainQueryFilterRange::Unbounded,
                        entry_type: None,
                        entry_hashes: None,
                        action_type: None,
                        include_entries: false,
                        order_descending: false,
                    },
                    holochain_p2p::actor::GetActivityOptions::default(),
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_agent_activity_with_unresponsive_agents() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());
    let unresponsive_handler = Arc::new(UnresponsiveHandler);

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), unresponsive_handler.clone(), &addr).await;
    let (_agent3, hc3, _) = spawn_test(dna_hash.clone(), unresponsive_handler, &addr).await;
    let (_agent4, hc4, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;
    hc3.test_set_full_arcs(space.clone()).await;
    hc4.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // If we get a response at all then at least one peer completed the request
            if hc1
                .get_agent_activity(
                    dna_hash.clone(),
                    AgentPubKey::from_raw_36(vec![2; 36]),
                    ChainQueryFilter {
                        sequence_range: ChainQueryFilterRange::Unbounded,
                        entry_type: None,
                        entry_hashes: None,
                        action_type: None,
                        include_entries: false,
                        order_descending: false,
                    },
                    holochain_p2p::actor::GetActivityOptions::default(),
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    })
    .await
    .unwrap();

    let requests = handler.calls.lock().unwrap();
    assert_eq!(*requests, ["get_agent_activity"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler, &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // if we get a response at all, the full back-n-forth succeeded
            if hc2
                .must_get_agent_activity(
                    dna_hash.clone(),
                    AgentPubKey::from_raw_36(vec![2; 36]),
                    ChainFilter {
                        chain_top: ActionHash::from_raw_36(vec![3; 36]),
                        limit_conditions: LimitConditions::ToGenesis,
                        include_cached_entries: false,
                    },
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_must_get_agent_activity_with_unresponsive_agents() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());
    let unresponsive_handler = Arc::new(UnresponsiveHandler);

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), unresponsive_handler.clone(), &addr).await;
    let (_agent3, hc3, _) = spawn_test(dna_hash.clone(), unresponsive_handler, &addr).await;
    let (_agent4, hc4, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;
    hc3.test_set_full_arcs(space.clone()).await;
    hc4.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // If we get a response at all then at least one peer completed the request
            if hc1
                .must_get_agent_activity(
                    dna_hash.clone(),
                    AgentPubKey::from_raw_36(vec![2; 36]),
                    ChainFilter {
                        chain_top: ActionHash::from_raw_36(vec![3; 36]),
                        limit_conditions: LimitConditions::ToGenesis,
                        include_cached_entries: false,
                    },
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    })
    .await
    .unwrap();

    let requests = handler.calls.lock().unwrap();
    assert_eq!(*requests, ["must_get_agent_activity"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_validation_receipts() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (agent1, _hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    let (_agent2, hc2, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    hc2.send_validation_receipts(
        dna_hash,
        agent1,
        <Vec<SignedValidationReceipt>>::new().into(),
    )
    .await
    .unwrap();

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            if let Some(res) = handler.calls.lock().unwrap().first() {
                assert_eq!("validation_receipts", res);
                break;
            }
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_authority_for_hash() {
    test_run();

    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;
    hc1.test_set_full_arcs(space.clone()).await;

    assert!(hc1
        .authority_for_hash(
            dna_hash,
            HoloHash::from_raw_36_and_type(vec![4; 36], holo_hash::hash_type::AnyLinkable::Entry)
        )
        .await
        .unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_target_arcs() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, _) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    let arcs = hc1.target_arcs(dna_hash).await.unwrap();
    assert_eq!(&[DhtArc::FULL][..], &arcs);
}

/// Note that this test does not prevent messages going via the network.
/// It just creates the conditions where we would expect a message to be bridged rather than sent
/// over the network. So if a check on that path prevents the message, this test would catch it.
#[tokio::test(flavor = "multi_thread")]
async fn bridged_call_remote() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, lair_client) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    let agent2 = lair_client.new_sign_keypair_random().await.unwrap();
    let local_agent2 = HolochainP2pLocalAgent::new(agent2.clone(), DhtArc::FULL, 1, lair_client);
    hc1.test_kitsune()
        .space(dna_hash.to_k2_space())
        .await
        .unwrap()
        .local_agent_join(Arc::new(local_agent2))
        .await
        .unwrap();

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // Make sure we know about both agents
            if hc1
                .peer_store(dna_hash.clone())
                .await
                .unwrap()
                .get_all()
                .await
                .unwrap()
                .len()
                == 2
            {
                break;
            }
        }
    })
    .await
    .unwrap();

    // Check that both peers have the same URL
    let all_peer_urls = hc1
        .peer_store(dna_hash.clone())
        .await
        .unwrap()
        .get_all()
        .await
        .unwrap()
        .into_iter()
        .map(|a| a.url.clone().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(2, all_peer_urls.len());
    assert_eq!(all_peer_urls[0], all_peer_urls[1]);

    // Then send a remote call to the other local agent
    let resp = hc1
        .call_remote(
            dna_hash,
            agent2,
            ExternIO(b"world".to_vec()),
            Signature([0; 64]),
        )
        .await
        .unwrap();
    let resp: Vec<u8> = UnsafeBytes::from(resp).into();
    let resp = String::from_utf8_lossy(&resp);
    assert_eq!("got_call_remote: world", resp);
}

/// Note that this test does not prevent messages going via the network.
/// It just creates the conditions where we would expect a signal to be bridged rather than sent
/// over the network. So if a check on that path prevents the signal, this test would catch it.
#[tokio::test(flavor = "multi_thread")]
async fn bridged_remote_signal() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let handler = Arc::new(Handler::default());

    let (_bootstrap_srv, addr) = spawn_test_bootstrap().await.unwrap();
    let (_agent1, hc1, lair_client) = spawn_test(dna_hash.clone(), handler.clone(), &addr).await;

    let agent2 = lair_client.new_sign_keypair_random().await.unwrap();
    let local_agent2 = HolochainP2pLocalAgent::new(agent2.clone(), DhtArc::FULL, 1, lair_client);
    hc1.test_kitsune()
        .space(dna_hash.to_k2_space())
        .await
        .unwrap()
        .local_agent_join(Arc::new(local_agent2))
        .await
        .unwrap();

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;

            // Make sure we know about both agents
            if hc1
                .peer_store(dna_hash.clone())
                .await
                .unwrap()
                .get_all()
                .await
                .unwrap()
                .len()
                == 2
            {
                break;
            }
        }
    })
    .await
    .unwrap();

    // Check that both peers have the same URL
    let all_peer_urls = hc1
        .peer_store(dna_hash.clone())
        .await
        .unwrap()
        .get_all()
        .await
        .unwrap()
        .into_iter()
        .map(|a| a.url.clone().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(2, all_peer_urls.len());
    assert_eq!(all_peer_urls[0], all_peer_urls[1]);

    // Then send a remote call to the other local agent
    hc1.send_remote_signal(
        dna_hash,
        vec![(agent2, ExternIO(b"hello".to_vec()), Signature([0; 64]))],
    )
    .await
    .unwrap();

    tokio::time::timeout(UNRESPONSIVE_TIMEOUT, async {
        loop {
            if let Some(res) = handler.calls.lock().unwrap().first() {
                assert_eq!("got_call_remote: hello", res);
                break;
            }
            tokio::time::sleep(WAIT_BETWEEN_CALLS).await;
        }
    })
    .await
    .unwrap();
}

async fn spawn_test(
    dna_hash: DnaHash,
    handler: DynHcP2pHandler,
    bootstrap_addr: &SocketAddr,
) -> (AgentPubKey, actor::DynHcP2p, MetaLairClient) {
    let db_peer_meta =
        DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(dna_hash.clone()))).unwrap();
    let db_op = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();
    let conductor_db = DbWrite::test_in_mem(DbKindConductor).unwrap();
    let lair_client = test_keystore();

    let agent = lair_client.new_sign_keypair_random().await.unwrap();

    let hc = spawn_holochain_p2p(
        HolochainP2pConfig {
            get_db_peer_meta: Arc::new(move |_| {
                let db_peer_meta = db_peer_meta.clone();
                Box::pin(async move { Ok(db_peer_meta.clone()) })
            }),
            get_db_op_store: Arc::new(move |_| {
                let db_op = db_op.clone();
                Box::pin(async move { Ok(db_op.clone()) })
            }),
            get_conductor_db: Arc::new(move || {
                let conductor_db = conductor_db.clone();
                Box::pin(async move { conductor_db })
            }),
            k2_test_builder: false,
            network_config: Some(serde_json::json!({
                "coreBootstrap": {
                    "serverUrl": format!("http://{bootstrap_addr}"),
                },
                "tx5Transport": {
                    "serverUrl": format!("ws://{bootstrap_addr}"),
                    "signalAllowPlainText": true,
                }
            })),
            request_timeout: Duration::from_secs(3),
            ..Default::default()
        },
        lair_client.clone(),
    )
    .await
    .unwrap();

    hc.register_handler(handler).await.unwrap();

    hc.join(dna_hash.clone(), agent.clone(), None)
        .await
        .unwrap();

    // TODO: Wait until the peer sees itself in the peer store.
    // This shouldn't be necessary, because preflight should come with the agent infos
    // required to establish a connection.
    retry_fn_until_timeout(
        || async {
            hc.peer_store(dna_hash.clone())
                .await
                .unwrap()
                .get(agent.to_k2_agent())
                .await
                .unwrap()
                .is_some()
        },
        Some(100),
        Some(1),
    )
    .await
    .unwrap();

    (agent, hc, lair_client)
}
