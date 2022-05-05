use kitsune_p2p_types::box_fut;

use super::*;

/// A supertrait of KitsuneHost convenient for defining test handlers.
/// Allows only specifying the methods you care about, and letting all the rest
/// throw errors if called
pub trait KitsuneHostDefaultError: KitsuneHost {
    /// Name to be printed out on unimplemented error
    const NAME: &'static str;

    /// We need to get previously stored agent info.
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

    /// Extrapolated Peer Coverage
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

    /// Record a set of metric records
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
}
