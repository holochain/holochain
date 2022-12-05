use kitsune_p2p_types::box_fut;
use kitsune_p2p_types::dht::region_set::RegionSetLtcs;

use super::*;

/// A supertrait of KitsuneHost convenient for defining test handlers.
/// Allows only specifying the methods you care about, and letting all the rest
/// throw errors if called
#[allow(missing_docs)]
pub trait KitsuneHostDefaultError: KitsuneHost + FetchQueueConfig {
    /// Name to be printed out on unimplemented error
    const NAME: &'static str;

    fn get_agent_info_signed(
        &self,
        _input: GetAgentInfoSignedEvt,
    ) -> KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        box_fut(Err(format!(
            "error for unimplemented KitsuneHost test behavior: method {} of {}",
            "get_agent_info_signed",
            Self::NAME
        )
        .into()))
    }

    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<KitsuneSpace>,
        _dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        box_fut(Err(format!(
            "error for unimplemented KitsuneHost test behavior: method {} of {}",
            "peer_extrapolated_coverage",
            Self::NAME
        )
        .into()))
    }

    fn record_metrics(
        &self,
        _space: Arc<KitsuneSpace>,
        _records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()> {
        box_fut(Err(format!(
            "error for unimplemented KitsuneHost test behavior: method {} of {}",
            "record_metrics",
            Self::NAME
        )
        .into()))
    }

    fn query_region_set(
        &self,
        _space: Arc<KitsuneSpace>,
        _dht_arc_set: Arc<DhtArcSet>,
    ) -> KitsuneHostResult<RegionSetLtcs> {
        box_fut(Err(format!(
            "error for unimplemented KitsuneHost test behavior: method {} of {}",
            "query_region_set",
            Self::NAME
        )
        .into()))
    }

    /// Given an input list of regions, return a list of equal or greater length
    /// such that each region's size is less than the `size_limit`, by recursively
    /// subdividing regions which are over the size limit.
    fn query_size_limited_regions(
        &self,
        _space: Arc<KitsuneSpace>,
        _size_limit: u32,
        _regions: Vec<Region>,
    ) -> KitsuneHostResult<Vec<Region>> {
        box_fut(Err(format!(
            "error for unimplemented KitsuneHost test behavior: method {} of {}",
            "query_size_limited_regions",
            Self::NAME
        )
        .into()))
    }

    fn get_topology(&self, _space: Arc<KitsuneSpace>) -> KitsuneHostResult<Topology> {
        box_fut(Err(format!(
            "error for unimplemented KitsuneHost test behavior: method {} of {}",
            "get_topology",
            Self::NAME
        )
        .into()))
    }

    fn op_hash(&self, _op_data: KOpData) -> KitsuneHostResult<KOpHash> {
        box_fut(Err(format!(
            "error for unimplemented KitsuneHost test behavior: method {} of {}",
            "op_hash",
            Self::NAME
        )
        .into()))
    }

    fn query_op_hashes_by_region(
        &self,
        _space: Arc<KitsuneSpace>,
        _region: RegionCoords,
    ) -> KitsuneHostResult<Vec<OpHashSized>> {
        box_fut(Err(format!(
            "error for unimplemented KitsuneHost test behavior: method {} of {}",
            "query_op_hashes_by_region",
            Self::NAME
        )
        .into()))
    }

    fn merge_fetch_contexts(&self, _a: u32, _b: u32) -> u32 {
        0
    }
}

impl<T: KitsuneHostDefaultError> KitsuneHost for T {
    fn get_agent_info_signed(
        &self,
        input: GetAgentInfoSignedEvt,
    ) -> KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        KitsuneHostDefaultError::get_agent_info_signed(self, input)
    }

    fn peer_extrapolated_coverage(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        KitsuneHostDefaultError::peer_extrapolated_coverage(self, space, dht_arc_set)
    }

    fn record_metrics(
        &self,
        space: Arc<KitsuneSpace>,
        records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()> {
        KitsuneHostDefaultError::record_metrics(self, space, records)
    }

    fn query_size_limited_regions(
        &self,
        space: Arc<KitsuneSpace>,
        size_limit: u32,
        regions: Vec<Region>,
    ) -> crate::KitsuneHostResult<Vec<Region>> {
        KitsuneHostDefaultError::query_size_limited_regions(self, space, size_limit, regions)
    }

    fn query_region_set(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: Arc<DhtArcSet>,
    ) -> KitsuneHostResult<RegionSetLtcs> {
        KitsuneHostDefaultError::query_region_set(self, space, dht_arc_set)
    }

    fn get_topology(&self, space: Arc<KitsuneSpace>) -> KitsuneHostResult<Topology> {
        KitsuneHostDefaultError::get_topology(self, space)
    }

    fn op_hash(&self, op_data: KOpData) -> KitsuneHostResult<KOpHash> {
        KitsuneHostDefaultError::op_hash(self, op_data)
    }

    fn query_op_hashes_by_region(
        &self,
        space: Arc<KitsuneSpace>,
        region: RegionCoords,
    ) -> KitsuneHostResult<Vec<OpHashSized>> {
        KitsuneHostDefaultError::query_op_hashes_by_region(self, space, region)
    }
}
