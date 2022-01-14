//! Represents data structures outside of kitsune, on the "host" side,
//! i.e. methods which would be called via ghost_actors, i.e. Holochain.

// mod op_store;

// pub use op_store::*;

use std::sync::Arc;

use crate::{
    agent::AgentInfo,
    coords::{SpacetimeCoords, Topology},
    hash::AgentKey,
    op::{Op, OpData, OpRegion, Timestamp},
    region::RegionBounds,
    region::RegionData,
    Loc,
};

/// TODO: make async
pub trait AccessOpStore<D = RegionData, O: OpRegion<D> = OpData> {
    fn query_op_data(&self, region: &RegionBounds) -> Vec<Arc<O>>;

    fn query_region_data(&self, region: &RegionBounds) -> D;

    fn integrate_ops<Ops: Clone + Iterator<Item = Arc<O>>>(&mut self, ops: Ops);

    fn integrate_op(&mut self, op: Arc<O>) {
        self.integrate_ops([op].into_iter())
    }
}

/// TODO: make async
pub trait AccessPeerStore {
    fn get_agent_info(&self, agent: AgentKey) -> AgentInfo;
}
