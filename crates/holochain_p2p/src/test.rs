use crate::actor::*;
use crate::HolochainP2pDna;
use crate::*;
use ::fixt::prelude::*;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_nonce::Nonce256Bits;
use holochain_zome_types::fixt::ActionFixturator;
use kitsune2_api::{
    Builder, Config, DhtArc, DynFetch, DynPeerStore, DynPublish, DynTransport, K2Result, OpId,
    Publish, PublishFactory, SpaceId, Url,
};

#[derive(Debug)]
struct StubNetwork;

#[allow(unused_variables)]
impl HcP2p for StubNetwork {
    #[cfg(feature = "test_utils")]
    fn test_kitsune(&self) -> &kitsune2_api::DynKitsune {
        unimplemented!()
    }

    fn peer_store(
        &self,
        dna_hash: DnaHash,
    ) -> BoxFut<'_, HolochainP2pResult<kitsune2_api::DynPeerStore>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn register_handler(
        &self,
        handler: event::DynHcP2pHandler,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn join(
        &self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        maybe_agent_info: Option<AgentInfoSigned>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn leave(
        &self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn new_integrated_data(
        &self,
        space_id: kitsune2_api::SpaceId,
        ops: Vec<kitsune2_api::StoredOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn call_remote(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn send_remote_signal(
        &self,
        dna_hash: DnaHash,
        to_agent_list: Vec<(AgentPubKey, ExternIO, Signature)>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn publish(
        &self,
        dna_hash: DnaHash,
        request_validation_receipt: bool,
        countersigning_session: bool,
        basis_hash: holo_hash::OpBasis,
        source: AgentPubKey,
        op_hash_list: Vec<DhtOpHash>,
        timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn publish_countersign(
        &self,
        dna_hash: DnaHash,
        flag: bool,
        basis_hash: holo_hash::OpBasis,
        op: DhtOp,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn get(
        &self,
        dna_hash: DnaHash,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<WireOps>>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn get_meta(
        &self,
        dna_hash: DnaHash,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<MetadataSet>>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn get_links(
        &self,
        dna_hash: DnaHash,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<WireLinkOps>>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn count_links(
        &self,
        dna_hash: DnaHash,
        query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn get_agent_activity(
        &self,
        dna_hash: DnaHash,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<AgentActivityResponse>>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn must_get_agent_activity(
        &self,
        dna_hash: DnaHash,
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<MustGetAgentActivityResponse>>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn send_validation_receipts(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn authority_for_hash(
        &self,
        dna_hash: DnaHash,
        basis_hash: OpBasis,
    ) -> BoxFut<'_, HolochainP2pResult<bool>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn countersigning_session_negotiation(
        &self,
        dna_hash: DnaHash,
        agents: Vec<AgentPubKey>,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn dump_network_metrics(
        &self,
        dna_hash: Option<DnaHash>,
    ) -> BoxFut<'_, HolochainP2pResult<String>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn dump_network_stats(&self) -> BoxFut<'_, HolochainP2pResult<String>> {
        Box::pin(async { Err("stub".into()) })
    }

    fn target_arcs(&self, dna_hash: DnaHash) -> BoxFut<'_, HolochainP2pResult<Vec<DhtArc>>> {
        Box::pin(async { Err("stub".into()) })
    }
}

/// Spawn a stub network that doesn't respond to any messages.
/// Use `test_network()` if you want a real test network.
pub async fn stub_network() -> DynHcP2p {
    Arc::new(StubNetwork)
}

fixturator!(
    HolochainP2pDna;
    curve Empty {
        tokio_helper::block_forever_on(async {
            let holochain_p2p = crate::test::stub_network().await;
            HolochainP2pDna::new(
                holochain_p2p,
                DnaHashFixturator::new(Empty).next().unwrap(),
                None
            )
        })
    };
    curve Unpredictable {
        HolochainP2pDnaFixturator::new(Empty).next().unwrap()
    };
    curve Predictable {
        HolochainP2pDnaFixturator::new(Empty).next().unwrap()
    };
);

#[derive(Debug)]
pub struct NoopPublish;

impl Publish for NoopPublish {
    fn publish_ops(&self, _op_ids: Vec<OpId>, _target: Url) -> BoxFut<'_, K2Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn publish_agent(
        &self,
        _agent_info: Arc<AgentInfoSigned>,
        _target: Url,
    ) -> BoxFut<'_, K2Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Debug)]
pub struct NoopPublishFactory;

impl PublishFactory for NoopPublishFactory {
    fn default_config(&self, _config: &mut Config) -> K2Result<()> {
        Ok(())
    }

    fn validate_config(&self, _config: &Config) -> K2Result<()> {
        Ok(())
    }

    fn create(
        &self,
        _builder: Arc<Builder>,
        _space_id: SpaceId,
        _fetch: DynFetch,
        _peer_store: DynPeerStore,
        _transport: DynTransport,
    ) -> BoxFut<'static, K2Result<DynPublish>> {
        Box::pin(async {
            let instance: DynPublish = Arc::new(NoopPublish);
            Ok(instance)
        })
    }
}
