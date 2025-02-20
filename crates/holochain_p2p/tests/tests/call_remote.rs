use holochain_keystore::*;
use holochain_p2p::event::*;
use holochain_p2p::*;
use holochain_types::prelude::*;
use kitsune2_api::*;
use std::sync::Arc;

#[derive(Debug)]
struct Handler;

impl HcP2pHandler for Handler {
    fn call_remote(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _zome_call_params_serialized: ExternIO,
        _signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>> {
        Box::pin(async move { todo!() })
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
        Box::pin(async move { todo!() })
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

    let (agent1, hc1) = spawn_test(dna_hash.clone()).await;
    let (agent2, hc2) = spawn_test(dna_hash.clone()).await;

    hc1.call_remote(
        dna_hash,
        agent2,
        ExternIO(b"hello".to_vec()),
        Signature([0; 64]),
    )
    .await
    .unwrap();
}

async fn spawn_test(dna_hash: DnaHash) -> (AgentPubKey, actor::DynHcP2p) {
    let space_id = dna_hash.to_k2_space();
    let db_peer_meta = DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(space_id))).unwrap();
    let db_op = DbWrite::test_in_mem(DbKindDht(Arc::new(dna_hash.clone()))).unwrap();
    let lair_client = test_keystore();
    let handler: DynHcP2pHandler = Arc::new(Handler);

    let agent = lair_client.new_sign_keypair_random().await.unwrap();
    println!("{agent:?}");

    let hc = spawn_holochain_p2p(
        HolochainP2pConfig {
            k2_test_builder: true,
            ..Default::default()
        },
        db_peer_meta,
        db_op,
        handler,
        lair_client,
    )
    .await
    .unwrap();

    hc.join(dna_hash, agent.clone(), None).await.unwrap();

    (agent, hc)
}
