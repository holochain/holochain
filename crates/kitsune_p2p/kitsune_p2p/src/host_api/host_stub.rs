use super::*;
use crate::KitsuneHostDefaultError;
use kitsune_p2p_fetch::*;

/// Signature for check_op_data_impl
pub type CheckOpDataImpl = Box<
    dyn Fn(
            Arc<KitsuneSpace>,
            Vec<KOpHash>,
            Option<FetchContext>,
        ) -> KitsuneHostResult<'static, Vec<bool>>
        + 'static
        + Send
        + Sync,
>;

struct HostStubErr;

impl KitsuneHostDefaultError for HostStubErr {
    const NAME: &'static str = "HostStub";
}

impl FetchPoolConfig for HostStubErr {
    fn merge_fetch_contexts(&self, _a: u32, _b: u32) -> u32 {
        unimplemented!()
    }
}

/// Dummy host impl for plumbing
pub struct HostStub {
    err: HostStubErr,
    check_op_data_impl: Option<CheckOpDataImpl>,
}

impl HostStub {
    /// Constructor
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            err: HostStubErr,
            check_op_data_impl: None,
        })
    }

    /// Constructor
    pub fn with_check_op_data(check_op_data_impl: CheckOpDataImpl) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            err: HostStubErr,
            check_op_data_impl: Some(check_op_data_impl),
        })
    }
}

impl KitsuneHost for HostStub {
    fn block(&self, input: kitsune_p2p_block::Block) -> crate::KitsuneHostResult<()> {
        KitsuneHostDefaultError::block(&self.err, input)
    }

    fn unblock(&self, input: kitsune_p2p_block::Block) -> crate::KitsuneHostResult<()> {
        KitsuneHostDefaultError::unblock(&self.err, input)
    }

    fn get_agent_info_signed(
        &self,
        input: GetAgentInfoSignedEvt,
    ) -> KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        KitsuneHostDefaultError::get_agent_info_signed(&self.err, input)
    }

    fn remove_agent_info_signed(&self, input: GetAgentInfoSignedEvt) -> KitsuneHostResult<bool> {
        KitsuneHostDefaultError::remove_agent_info_signed(&self.err, input)
    }

    fn peer_extrapolated_coverage(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        KitsuneHostDefaultError::peer_extrapolated_coverage(&self.err, space, dht_arc_set)
    }

    fn record_metrics(
        &self,
        space: Arc<KitsuneSpace>,
        records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()> {
        KitsuneHostDefaultError::record_metrics(&self.err, space, records)
    }

    fn query_size_limited_regions(
        &self,
        space: Arc<KitsuneSpace>,
        size_limit: u32,
        regions: Vec<Region>,
    ) -> crate::KitsuneHostResult<Vec<Region>> {
        KitsuneHostDefaultError::query_size_limited_regions(&self.err, space, size_limit, regions)
    }

    fn query_region_set(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: Arc<DhtArcSet>,
    ) -> KitsuneHostResult<RegionSetLtcs> {
        KitsuneHostDefaultError::query_region_set(&self.err, space, dht_arc_set)
    }

    fn get_topology(&self, space: Arc<KitsuneSpace>) -> KitsuneHostResult<Topology> {
        KitsuneHostDefaultError::get_topology(&self.err, space)
    }

    fn op_hash(&self, op_data: KOpData) -> KitsuneHostResult<KOpHash> {
        KitsuneHostDefaultError::op_hash(&self.err, op_data)
    }

    fn query_op_hashes_by_region(
        &self,
        space: Arc<KitsuneSpace>,
        region: RegionCoords,
    ) -> KitsuneHostResult<Vec<OpHashSized>> {
        KitsuneHostDefaultError::query_op_hashes_by_region(&self.err, space, region)
    }

    fn check_op_data(
        &self,
        space: Arc<KitsuneSpace>,
        op_hash_list: Vec<KOpHash>,
        context: Option<kitsune_p2p_fetch::FetchContext>,
    ) -> KitsuneHostResult<Vec<bool>> {
        if let Some(i) = &self.check_op_data_impl {
            i(space, op_hash_list, context)
        } else {
            KitsuneHost::check_op_data(&self.err, space, op_hash_list, context)
        }
    }
}

impl FetchPoolConfig for HostStub {
    fn merge_fetch_contexts(&self, _a: u32, _b: u32) -> u32 {
        unimplemented!()
    }
}
