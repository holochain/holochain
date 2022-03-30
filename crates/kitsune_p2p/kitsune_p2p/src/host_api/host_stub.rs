use std::sync::Arc;

use kitsune_p2p_types::{bin_types::KitsuneSpace, box_fut, dht_arc::DhtArcSet};

use crate::{KitsuneHostPanicky, KitsuneHostResult};

/// Dummy host impl for plumbing
pub struct HostStub;

impl KitsuneHostPanicky for HostStub {
    const NAME: &'static str = "HostStub";

    // XXX: shouldn't be implemented for a stub, but we have tests failing due to this
    // being called, so let's just implement it in a naive way.
    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<KitsuneSpace>,
        _dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        box_fut(Ok(vec![1.0]))
    }
}

impl HostStub {
    /// Constructor
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self)
    }
}
