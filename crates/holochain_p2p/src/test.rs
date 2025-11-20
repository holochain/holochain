use crate::actor::*;
use crate::HolochainP2pDna;
use crate::*;
use ::fixt::prelude::*;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_nonce::Nonce256Bits;
use holochain_zome_types::fixt::ActionFixturator;
use kitsune2_api::DynGossip;
use kitsune2_api::DynLocalAgentStore;
use kitsune2_api::DynOpStore;
use kitsune2_api::DynPeerMetaStore;
use kitsune2_api::Gossip;
use kitsune2_api::GossipFactory;
use kitsune2_api::GossipStateSummary;
use kitsune2_api::GossipStateSummaryRequest;
use kitsune2_api::{
    Bootstrap, BootstrapFactory, Builder, Config, DhtArc, DynBootstrap, DynFetch, DynPeerStore,
    DynPublish, DynTransport, K2Result, OpId, Publish, PublishFactory, SpaceId, Url,
};
use std::collections::HashMap;

/// Spawn a stub network that doesn't respond to any messages.
pub async fn stub_network() -> DynHcP2p {
    Arc::new(MockHcP2p::new())
}

#[derive(Debug)]
pub struct NoopGossip;

impl Gossip for NoopGossip {
    fn get_state_summary(
        &self,
        _request: GossipStateSummaryRequest,
    ) -> BoxFut<'_, K2Result<GossipStateSummary>> {
        Box::pin(async {
            Ok(GossipStateSummary {
                accepted_rounds: Default::default(),
                dht_summary: Default::default(),
                initiated_round: Default::default(),
                peer_meta: Default::default(),
            })
        })
    }

    fn inform_ops_stored(&self, ops: Vec<StoredOp>) -> BoxFut<'_, K2Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Debug)]
pub struct NoopGossipFactory;

impl GossipFactory for NoopGossipFactory {
    fn default_config(&self, _config: &mut Config) -> K2Result<()> {
        Ok(())
    }

    fn validate_config(&self, _config: &Config) -> K2Result<()> {
        Ok(())
    }

    fn create(
        &self,
        builder: Arc<Builder>,
        space_id: SpaceId,
        peer_store: DynPeerStore,
        local_agent_store: DynLocalAgentStore,
        peer_meta_store: DynPeerMetaStore,
        op_store: DynOpStore,
        transport: DynTransport,
        fetch: DynFetch,
    ) -> BoxFut<'static, K2Result<DynGossip>> {
        Box::pin(async {
            let instance: DynGossip = Arc::new(NoopGossip);
            Ok(instance)
        })
    }
}

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
        _peer_meta_store: DynPeerMetaStore,
        _transport: DynTransport,
    ) -> BoxFut<'static, K2Result<DynPublish>> {
        Box::pin(async {
            let instance: DynPublish = Arc::new(NoopPublish);
            Ok(instance)
        })
    }
}

#[derive(Debug)]
pub struct NoopBootstrap;

impl Bootstrap for NoopBootstrap {
    fn put(&self, _info: Arc<AgentInfoSigned>) {
        // Do nothing
    }
}

#[derive(Debug)]
pub struct NoopBootstrapFactory;

impl BootstrapFactory for NoopBootstrapFactory {
    fn default_config(&self, _config: &mut Config) -> K2Result<()> {
        Ok(())
    }

    fn validate_config(&self, _config: &Config) -> K2Result<()> {
        Ok(())
    }

    fn create(
        &self,
        _builder: Arc<Builder>,
        _peer_store: DynPeerStore,
        _space_id: SpaceId,
    ) -> BoxFut<'static, K2Result<DynBootstrap>> {
        Box::pin(async {
            let instance: DynBootstrap = Arc::new(NoopBootstrap);
            Ok(instance)
        })
    }
}
