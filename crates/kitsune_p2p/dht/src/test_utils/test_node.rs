use std::ops::Add;

use crate::{
    agent::AgentInfo,
    arq::{Arq, ArqSet},
    coords::{RegionBounds, Topology},
    hash::{fake_hash, AgentKey},
    host::{AccessOpStore, AccessPeerStore},
    op::Op,
    region_data::RegionData,
    tree::Tree,
};

use super::op_store::OpStore;

pub struct TestNode {
    agent: AgentKey,
    agent_info: AgentInfo,
    ops: OpStore,
    tree: Tree,
}

impl TestNode {
    pub fn new(topo: Topology, arq: Arq) -> Self {
        Self {
            agent: AgentKey(fake_hash()),
            agent_info: AgentInfo { arq },
            ops: OpStore::default(),
            tree: Tree::new(topo),
        }
    }

    pub fn arq_set(&self) -> ArqSet {
        ArqSet::single(self.agent_info.arq.to_bounds())
    }

    pub fn gossip_with(&mut self, other: &mut Self) {
        let mut stats = TestNodeGossipRoundStats::default();

        // 1. calculate common arqset
        let common_arqs = self.arq_set().intersection(&other.arq_set());

        // 2. calculate regions
        stats.region_data_sent += todo!();
        stats.region_data_rcvd += todo!();

        // 3. send regions

        // 4. send ops
        stats.op_data_sent += todo!();
        stats.op_data_rcvd += todo!();
    }
}

#[derive(Clone, Debug, Default)]
pub struct TestNodeGossipRoundStats {
    region_data_sent: u32,
    region_data_rcvd: u32,
    op_data_sent: u32,
    op_data_rcvd: u32,
}

impl TestNodeGossipRoundStats {
    pub fn total_sent(&self) -> u32 {
        self.region_data_sent + self.op_data_sent
    }

    pub fn total_rcvd(&self) -> u32 {
        self.region_data_rcvd + self.op_data_rcvd
    }
}

impl AccessOpStore for TestNode {
    fn query_op_data(&self, region: &RegionBounds) -> Vec<Op> {
        self.ops.query_region(region)
    }

    fn query_region_data(&self, region: &RegionBounds) -> RegionData {
        self.tree.lookup(region)
    }

    fn integrate_op(&mut self, op: Op) {
        self.tree.add_op(op);
    }
}

impl AccessPeerStore for TestNode {
    fn get_agent_info(&self, _agent: AgentKey) -> crate::agent::AgentInfo {
        self.agent_info.clone()
    }
}
