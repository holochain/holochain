//! Represents data structures outside of kitsune, on the "host" side,
//! i.e. methods which would be called via ghost_actors, i.e. Holochain.
//!
//! XXX: These traits are an artifact of the original prototype of the quantized gossip
//! algorithm, which started as its own crate separate from the rest of Kitsune,
//! and created these traits to build up a suitable interface from scratch.
//! Eventually these should be better integrated with the existing KitsuneP2pEvent
//! interface, and the KitsuneHost interface: the integration of those two is
//! still a work in progress, and these traits should be considered as part of
//! that work.

use std::sync::Arc;

use must_future::MustBoxFuture;

use crate::{
    arq::ArqBoundsSet,
    hash::AgentKey,
    op::*,
    region::*,
    region_set::*,
    spacetime::{GossipParams, TelescopingTimes, TimeQuantum, Topology},
    test_utils::OpData,
    Arq,
};

/// All methods involved in accessing the op store, to be implemented by the host.
// TODO: make async
pub trait AccessOpStore<O: OpRegion<D>, D: RegionDataConstraints = RegionData>: Send {
    /// Query the actual ops inside a region
    fn query_op_data(&self, region: &RegionCoords) -> Vec<Arc<O>>;

    /// Query the RegionData of a region, including the hash of all ops, size, and count
    fn query_region_data(&self, region: &RegionCoords) -> D;

    /// Fetch a set of Regions (the coords and the data) given the set of coords
    fn fetch_region_set(
        &self,
        coords: RegionCoordSetLtcs,
    ) -> MustBoxFuture<Result<RegionSetLtcs<D>, ()>>;

    /// Integrate incoming ops, updating the necessary stores
    fn integrate_ops<Ops: Clone + Iterator<Item = Arc<O>>>(&mut self, ops: Ops);

    /// Integrate a single op
    fn integrate_op(&mut self, op: Arc<O>) {
        self.integrate_ops([op].into_iter())
    }

    /// Get the GossipParams associated with this store
    fn gossip_params(&self) -> GossipParams;

    /// Get the Topology associated with this store
    fn topo(&self) -> &Topology;

    /// Get the RegionSet for this node, suitable for gossiping
    fn region_set(&self, arq_set: ArqBoundsSet, now: TimeQuantum) -> RegionSet<D> {
        let coords = RegionCoordSetLtcs::new(TelescopingTimes::new(now), arq_set);
        coords
            .into_region_set_infallible_unlocked(|(_, coords)| self.query_region_data(&coords))
            .into()
    }
}

/// All methods involved in accessing the peer store, to be implemented by the host.
// TODO: make async
pub trait AccessPeerStore {
    /// Get the arq for an agent
    fn get_agent_arq(&self, agent: &AgentKey) -> Arq;

    /// Get the set of all arqs for this node
    fn get_arq_set(&self) -> ArqBoundsSet;
}

/// Represents all methods implemented by the host.
pub trait HostAccess<O: OpRegion<D>, D: RegionDataConstraints = RegionData>:
    AccessOpStore<O, D> + AccessPeerStore
{
}
impl<T, O: OpRegion<D>, D: RegionDataConstraints> HostAccess<O, D> for T where
    T: AccessOpStore<O, D> + AccessPeerStore
{
}

/// Represents all methods implemented by the host.
pub trait HostAccessTest: HostAccess<OpData, RegionData> {}
impl<T> HostAccessTest for T where T: HostAccess<OpData, RegionData> {}
