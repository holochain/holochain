//! Represents data structures outside of kitsune, on the "host" side,
//! i.e. methods which would be called via ghost_actors, i.e. Holochain.

// mod op_store;

// pub use op_store::*;

use std::sync::Arc;

use crate::{
    agent::AgentInfo,
    arq::ArqBoundsSet,
    hash::AgentKey,
    op::*,
    quantum::{GossipParams, TelescopingTimes, TimeQuantum, Topology},
    region::*,
    tree::TreeDataConstraints,
};

/// TODO: make async
pub trait AccessOpStore<D: TreeDataConstraints = RegionData, O: OpRegion<D> = OpData> {
    fn query_op_data(&self, region: &RegionBounds) -> Vec<Arc<O>>;
    fn query_ops_by_coords(&self, region: &RegionCoords) -> Vec<Arc<O>> {
        self.query_op_data(&region.to_bounds(self.topo()))
    }

    fn query_region(&self, region: &RegionBounds) -> D;

    fn query_region_coords(&self, region: &RegionCoords) -> D {
        self.query_region(&region.to_bounds(self.topo()))
    }

    fn integrate_ops<Ops: Clone + Iterator<Item = Arc<O>>>(&mut self, ops: Ops);

    fn integrate_op(&mut self, op: Arc<O>) {
        self.integrate_ops([op].into_iter())
    }

    fn gossip_params(&self) -> GossipParams;
    fn topo(&self) -> &Topology;

    /// Get the RegionSet for this node, suitable for gossiping
    fn region_set(&self, arq_set: ArqBoundsSet, now: TimeQuantum) -> RegionSet<D> {
        let coords = RegionCoordSetXtcs::new(TelescopingTimes::new(now), arq_set);
        let data = coords
            .region_coords_nested()
            .map(|columns| {
                columns
                    .map(|(_, coords)| self.query_region_coords(&coords))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        RegionSetXtcs::from_data(coords, data).into()
    }
}

/// TODO: make async
pub trait AccessPeerStore {
    fn get_agent_info(&self, agent: AgentKey) -> AgentInfo;

    fn get_arq_set(&self) -> ArqBoundsSet;
}

// TODO: eventually move any methods out of here and into a component trait
pub trait HostAccess<D: TreeDataConstraints = RegionData, O: OpRegion<D> = OpData>:
    AccessOpStore<D, O> + AccessPeerStore
{
}
impl<T, D: TreeDataConstraints, O: OpRegion<D>> HostAccess<D, O> for T where
    T: AccessOpStore<D, O> + AccessPeerStore
{
}
