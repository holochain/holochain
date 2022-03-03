use kitsune_p2p_types::dht::region::RegionSetXtcs;

use super::*;

/// A supertrait of KitsuneHost convenient for defining test handlers.
/// Allows only specifying the methods you care about, and letting all the rest
/// panic if called
#[allow(missing_docs)]
pub trait KitsuneHostPanicky: KitsuneHost {
    /// Name to be printed out on unimplemented panic
    const NAME: &'static str;

    fn get_agent_info_signed(
        &self,
        _input: GetAgentInfoSignedEvt,
    ) -> KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        unimplemented!(
            "default panic for unimplemented KitsuneHost test behavior: {}",
            Self::NAME
        )
    }

    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<KitsuneSpace>,
        _dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        unimplemented!(
            "default panic for unimplemented KitsuneHost test behavior: {}",
            Self::NAME
        )
    }

    fn record_metrics(
        &self,
        _space: Arc<KitsuneSpace>,
        _records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()> {
        unimplemented!(
            "default panic for unimplemented KitsuneHost test behavior: {}",
            Self::NAME
        )
    }

    fn query_region_set(
        &self,
        _space: &KitsuneSpace,
        _dht_arc_set: Arc<DhtArcSet>,
    ) -> KitsuneHostResult<RegionSetXtcs> {
        unimplemented!(
            "default panic for unimplemented KitsuneHost test behavior: {}",
            Self::NAME
        )
    }

    fn get_topology(&self, space: Arc<KitsuneSpace>) -> KitsuneHostResult<Option<Topology>> {
        unimplemented!(
            "default panic for unimplemented KitsuneHost test behavior: {}",
            Self::NAME
        )
    }
}

impl<T: KitsuneHostPanicky> KitsuneHost for T {
    fn get_agent_info_signed(
        &self,
        input: GetAgentInfoSignedEvt,
    ) -> KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        KitsuneHostPanicky::get_agent_info_signed(self, input)
    }

    fn peer_extrapolated_coverage(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        KitsuneHostPanicky::peer_extrapolated_coverage(self, space, dht_arc_set)
    }

    fn record_metrics(
        &self,
        space: Arc<KitsuneSpace>,
        records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()> {
        KitsuneHostPanicky::record_metrics(self, space, records)
    }

    fn query_region_set(
        &self,
        space: &KitsuneSpace,
        dht_arc_set: Arc<DhtArcSet>,
    ) -> KitsuneHostResult<RegionSetXtcs> {
        KitsuneHostPanicky::query_region_set(self, space, dht_arc_set)
    }

    fn get_topology(&self, space: Arc<KitsuneSpace>) -> KitsuneHostResult<Option<Topology>> {
        KitsuneHostPanicky::get_topology(self, space)
    }
}
