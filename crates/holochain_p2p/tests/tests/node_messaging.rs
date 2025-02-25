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
    fn call_remote(
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

    fn publish(
        &self,
        _dna_hash: DnaHash,
        _request_validation_receipt: bool,
        _countersigning_session: bool,
        _ops: Vec<holochain_types::dht_op::DhtOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }

    fn get(
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

    fn get_meta(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: GetMetaOptions,
    ) -> BoxFut<'_, HolochainP2pResult<MetadataSet>> {
        Box::pin(async move { todo!() })
    }

    fn get_links(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _link_key: WireLinkKey,
        _options: GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireLinkOps>> {
        Box::pin(async move { todo!() })
    }

    fn count_links(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        Box::pin(async move { todo!() })
    }

    fn get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _agent: AgentPubKey,
        _query: ChainQueryFilter,
        _options: GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<AgentActivityResponse>> {
        Box::pin(async move { todo!() })
    }

    fn must_get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _author: AgentPubKey,
        _filter: holochain_zome_types::chain::ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<MustGetAgentActivityResponse>> {
        Box::pin(async move { todo!() })
    }

    fn validation_receipts_received(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }

    fn countersigning_session_negotiation(
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

    // give some time for the full arcs to propagate
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // if we get a response at all, the full back-n-forth succeeded
    hc2
        .get(
            dna_hash,
            HoloHash::from_raw_36_and_type(vec![1; 36], holo_hash::hash_type::AnyDht::Entry),
            holochain_p2p::actor::GetOptions::default(),
        )
        .await
        .unwrap();
}

async fn spawn_test(dna_hash: DnaHash, handler: DynHcP2pHandler) -> (AgentPubKey, actor::DynHcP2p) {
    let space_id = dna_hash.to_k2_space();
    let db_peer_meta = DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(space_id))).unwrap();
    let db_op = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();
    let lair_client = test_keystore();

    let agent = lair_client.new_sign_keypair_random().await.unwrap();

    let hc = spawn_holochain_p2p(
        HolochainP2pConfig {
            get_db_peer_meta: Arc::new(move || {
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
        handler,
        lair_client,
    )
    .await
    .unwrap();

    hc.join(dna_hash, agent.clone(), None).await.unwrap();

    (agent, hc)
}
