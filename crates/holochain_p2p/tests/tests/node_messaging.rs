use holochain_keystore::*;
use holochain_p2p::event::*;
use holochain_p2p::*;
use holochain_types::prelude::*;
use kitsune2_api::*;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
struct Handler(pub Arc<Mutex<Vec<String>>>);

impl Default for Handler {
    fn default() -> Self {
        Handler(Arc::new(Mutex::new(Vec::new())))
    }
}

impl HcP2pHandler for Handler {
    fn handle_call_remote(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        _signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>> {
        Box::pin(async move {
            let respond = format!(
                "got_call_remote: {}",
                String::from_utf8_lossy(&zome_call_params_serialized.0),
            );
            self.0.lock().unwrap().push(respond.clone());
            Ok(UnsafeBytes::from(respond.into_bytes()).into())
        })
    }

    fn handle_publish(
        &self,
        _dna_hash: DnaHash,
        _request_validation_receipt: bool,
        _countersigning_session: bool,
        _ops: Vec<holochain_types::dht_op::DhtOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }

    fn handle_get(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: holochain_p2p::event::GetOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireOps>> {
        Box::pin(async move {
            self.0.lock().unwrap().push("get".into());
            let ops = WireOps::Entry(WireEntryOps::new());
            Ok(ops)
        })
    }

    fn handle_get_meta(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: GetMetaOptions,
    ) -> BoxFut<'_, HolochainP2pResult<MetadataSet>> {
        Box::pin(async move {
            self.0.lock().unwrap().push("get_meta".into());
            Ok(MetadataSet {
                actions: Default::default(),
                invalid_actions: Default::default(),
                deletes: Default::default(),
                updates: Default::default(),
                entry_dht_status: None,
            })
        })
    }

    fn handle_get_links(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _link_key: WireLinkKey,
        _options: GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireLinkOps>> {
        Box::pin(async move {
            self.0.lock().unwrap().push("get_links".into());
            Ok(WireLinkOps {
                creates: Vec::new(),
                deletes: Vec::new(),
            })
        })
    }

    fn handle_count_links(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        Box::pin(async move {
            self.0.lock().unwrap().push("count_links".into());
            Ok(CountLinksResponse::new(Vec::new()))
        })
    }

    fn handle_get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _agent: AgentPubKey,
        _query: ChainQueryFilter,
        _options: GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<AgentActivityResponse>> {
        Box::pin(async move {
            self.0.lock().unwrap().push("get_agent_activity".into());
            Ok(AgentActivityResponse {
                agent: AgentPubKey::from_raw_36(vec![2; 36]),
                valid_activity: ChainItems::NotRequested,
                rejected_activity: ChainItems::NotRequested,
                status: ChainStatus::Empty,
                highest_observed: None,
                warrants: Vec::new(),
            })
        })
    }

    fn handle_must_get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _author: AgentPubKey,
        _filter: holochain_zome_types::chain::ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<MustGetAgentActivityResponse>> {
        Box::pin(async move {
            self.0.lock().unwrap().push("get_agent_activity".into());
            Ok(MustGetAgentActivityResponse::EmptyRange)
        })
    }

    fn handle_validation_receipts_received(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            self.0.lock().unwrap().push("validation_receipts".into());
            Ok(())
        })
    }

    fn handle_countersigning_session_negotiation(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _message: CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_call_remote() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let handler = Arc::new(Handler::default());

    let (agent1, _hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
    let (_agent2, hc2) = spawn_test(dna_hash.clone(), handler).await;

    let resp = hc2
        .call_remote(
            dna_hash,
            agent1,
            ExternIO(b"hello".to_vec()),
            Signature([0; 64]),
        )
        .await
        .unwrap();
    let resp: Vec<u8> = UnsafeBytes::from(resp).into();
    let resp = String::from_utf8_lossy(&resp);
    assert_eq!("got_call_remote: hello", resp);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_signal() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let handler = Arc::new(Handler::default());

    let (agent1, _hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
    let (_agent2, hc2) = spawn_test(dna_hash.clone(), handler.clone()).await;

    hc2.send_remote_signal(
        dna_hash,
        vec![(agent1, ExternIO(b"hello".to_vec()), Signature([0; 64]))],
    )
    .await
    .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            if let Some(res) = handler.0.lock().unwrap().first() {
                assert_eq!("got_call_remote: hello", res);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
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

    let (_agent1, hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
    let (_agent2, hc2) = spawn_test(dna_hash.clone(), handler).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;

            // if we get a response at all, the full back-n-forth succeeded
            if hc2
                .get(
                    dna_hash.clone(),
                    HoloHash::from_raw_36_and_type(
                        vec![1; 36],
                        holo_hash::hash_type::AnyDht::Entry,
                    ),
                    holochain_p2p::actor::GetOptions::default(),
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
async fn test_get_meta() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_agent1, hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
    let (_agent2, hc2) = spawn_test(dna_hash.clone(), handler).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;

            // if we get a response at all, the full back-n-forth succeeded
            if hc2
                .get_meta(
                    dna_hash.clone(),
                    HoloHash::from_raw_36_and_type(
                        vec![1; 36],
                        holo_hash::hash_type::AnyDht::Entry,
                    ),
                    holochain_p2p::actor::GetMetaOptions::default(),
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
async fn test_get_links() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_agent1, hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
    let (_agent2, hc2) = spawn_test(dna_hash.clone(), handler).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;

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
async fn test_count_links() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_agent1, hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
    let (_agent2, hc2) = spawn_test(dna_hash.clone(), handler).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;

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
async fn test_get_agent_activity() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_agent1, hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
    let (_agent2, hc2) = spawn_test(dna_hash.clone(), handler).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;

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
async fn test_must_get_agent_activity() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_agent1, hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
    let (_agent2, hc2) = spawn_test(dna_hash.clone(), handler).await;

    hc1.test_set_full_arcs(space.clone()).await;
    hc2.test_set_full_arcs(space.clone()).await;

    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;

            // if we get a response at all, the full back-n-forth succeeded
            if hc2
                .must_get_agent_activity(
                    dna_hash.clone(),
                    AgentPubKey::from_raw_36(vec![2; 36]),
                    ChainFilter {
                        chain_top: ActionHash::from_raw_36(vec![3; 36]),
                        filters: ChainFilters::ToGenesis,
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
async fn test_validation_receipts() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let handler = Arc::new(Handler::default());

    let (agent1, _hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
    let (_agent2, hc2) = spawn_test(dna_hash.clone(), handler.clone()).await;

    hc2.send_validation_receipts(
        dna_hash,
        agent1,
        <Vec<SignedValidationReceipt>>::new().into(),
    )
    .await
    .unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            if let Some(res) = handler.0.lock().unwrap().first() {
                assert_eq!("validation_receipts", res);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_authority_for_hash() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_agent1, hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
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
async fn test_storage_arcs() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space = dna_hash.to_k2_space();
    let handler = Arc::new(Handler::default());

    let (_agent1, hc1) = spawn_test(dna_hash.clone(), handler.clone()).await;
    hc1.test_set_full_arcs(space.clone()).await;

    let arcs = hc1.storage_arcs(dna_hash).await.unwrap();
    assert_eq!(&[DhtArc::FULL][..], &arcs);
}

async fn spawn_test(dna_hash: DnaHash, handler: DynHcP2pHandler) -> (AgentPubKey, actor::DynHcP2p) {
    let db_peer_meta = DbWrite::test_in_mem(DbKindPeerMetaStore).unwrap();
    let db_op = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();
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
            k2_test_builder: true,
            ..Default::default()
        },
        lair_client,
    )
    .await
    .unwrap();

    hc.register_handler(handler).await.unwrap();

    hc.join(dna_hash, agent.clone(), None).await.unwrap();

    (agent, hc)
}
