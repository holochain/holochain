use std::sync::Arc;

use futures::FutureExt;
use kitsune_p2p::KitsuneHost;
use kitsune_p2p_types::agent_info::AgentInfoSigned;

#[derive(Debug, Clone)]
pub struct TestHost {
    agent_store: Arc<parking_lot::RwLock<Vec<AgentInfoSigned>>>,
}

impl TestHost {
    pub fn new(agent_store: Arc<parking_lot::RwLock<Vec<AgentInfoSigned>>>) -> Self {
        Self { agent_store }
    }
}

impl KitsuneHost for TestHost {
    fn block(&self, _input: kitsune_p2p_block::Block) -> kitsune_p2p::KitsuneHostResult<()> {
        todo!()
    }

    fn unblock(&self, _input: kitsune_p2p_block::Block) -> kitsune_p2p::KitsuneHostResult<()> {
        todo!()
    }

    fn is_blocked(
        &self,
        _input: kitsune_p2p_block::BlockTargetId,
        _timestamp: kitsune_p2p_types::dht::prelude::Timestamp,
    ) -> kitsune_p2p::KitsuneHostResult<bool> {
        // TODO implement me
        async move { Ok(false) }.boxed().into()
    }

    fn get_agent_info_signed(
        &self,
        input: kitsune_p2p::event::GetAgentInfoSignedEvt,
    ) -> kitsune_p2p::KitsuneHostResult<Option<AgentInfoSigned>> {
        let res = self
            .agent_store
            .read()
            .iter()
            .find(|p| p.agent == input.agent)
            .cloned();

        async move { Ok(res) }.boxed().into()
    }

    fn remove_agent_info_signed(
        &self,
        _input: kitsune_p2p::event::GetAgentInfoSignedEvt,
    ) -> kitsune_p2p::KitsuneHostResult<bool> {
        todo!()
    }

    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        _dht_arc_set: kitsune_p2p_types::dht_arc::DhtArcSet,
    ) -> kitsune_p2p::KitsuneHostResult<Vec<f64>> {
        // TODO implement me
        async move { Ok(vec![]) }.boxed().into()
    }

    fn query_region_set(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        _dht_arc_set: Arc<kitsune_p2p_types::dht_arc::DhtArcSet>,
    ) -> kitsune_p2p::KitsuneHostResult<kitsune_p2p_types::dht::prelude::RegionSetLtcs> {
        todo!()
    }

    fn query_size_limited_regions(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        _size_limit: u32,
        _regions: Vec<kitsune_p2p_types::dht::prelude::Region>,
    ) -> kitsune_p2p::KitsuneHostResult<Vec<kitsune_p2p_types::dht::prelude::Region>> {
        todo!()
    }

    fn query_op_hashes_by_region(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        _region: kitsune_p2p_types::dht::prelude::RegionCoords,
    ) -> kitsune_p2p::KitsuneHostResult<Vec<kitsune_p2p_fetch::OpHashSized>> {
        todo!()
    }

    fn record_metrics(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
        _records: Vec<kitsune_p2p_types::metrics::MetricRecord>,
    ) -> kitsune_p2p::KitsuneHostResult<()> {
        todo!()
    }

    fn get_topology(
        &self,
        _space: Arc<kitsune_p2p_bin_data::KitsuneSpace>,
    ) -> kitsune_p2p::KitsuneHostResult<kitsune_p2p_types::dht::prelude::Topology> {
        todo!()
    }

    fn op_hash(
        &self,
        _op_data: kitsune_p2p_types::KOpData,
    ) -> kitsune_p2p::KitsuneHostResult<kitsune_p2p_types::KOpHash> {
        todo!()
    }
}
