use std::sync::Arc;

use kitsune_p2p_types::{box_fut, dht_arc::DhtArcSet};

use crate::{event::MetricRecord, KitsuneHost, KitsuneHostResult};

/// Dummy host impl for plumbing
pub struct HostStub;

impl KitsuneHost for HostStub {
    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<crate::KitsuneSpace>,
        _dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        unreachable!("function not used in tests")
    }

    fn record_metrics(
        &self,
        _space: Arc<crate::KitsuneSpace>,
        _records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()> {
        box_fut(Ok(()))
    }
}

impl HostStub {
    /// Constructor
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self)
    }
}
