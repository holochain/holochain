//! Represents data structures outside of kitsune, on the "host" side,
//! i.e. methods which would be called via ghost_actors, i.e. Holochain.

// mod op_store;

// pub use op_store::*;

use crate::{
    agent::AgentInfo,
    coords::{RegionBounds, SpacetimeCoords, Topology},
    hash::AgentKey,
    op::{Op, Timestamp},
    region_data::RegionData,
    Loc,
};

pub trait AccessOpStore {
    fn query_op_data(&self, region: &RegionBounds) -> Vec<Op>;

    fn query_region_data(&self, region: &RegionBounds) -> RegionData;

    fn integrate_op(&mut self, op: Op);
}

pub trait AccessPeerStore {
    fn get_agent_info(&self, agent: AgentKey) -> AgentInfo;
}
