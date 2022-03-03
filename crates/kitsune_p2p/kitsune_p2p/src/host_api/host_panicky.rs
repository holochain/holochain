use super::*;

/// A supertrait of KitsuneHost convenient for defining test handlers.
/// Allows only specifying the methods you care about, and letting all the rest
/// panic if called
pub trait KitsuneHostPanicky: KitsuneHost {
    /// We need to get previously stored agent info.
    fn get_agent_info_signed(
        &self,
        _input: GetAgentInfoSignedEvt,
    ) -> KitsuneHostResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        unimplemented!("default panic for unimplemented KitsuneHost test behavior")
    }

    /// Extrapolated Peer Coverage
    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<KitsuneSpace>,
        _dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        unimplemented!("default panic for unimplemented KitsuneHost test behavior")
    }

    /// Record a set of metric records
    fn record_metrics(
        &self,
        _space: Arc<KitsuneSpace>,
        _records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()> {
        unimplemented!("default panic for unimplemented KitsuneHost test behavior")
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
}
