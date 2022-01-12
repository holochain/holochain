use std::ops::Add;

use kitsune_p2p_timestamp::Timestamp;

use crate::{
    agent::AgentInfo,
    arq::*,
    coords::Topology,
    hash::{fake_hash, AgentKey},
    host::{AccessOpStore, AccessPeerStore},
    op::Op,
    region::*,
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

    /// The ArqBounds to use when gossiping
    pub fn arq_bounds(&self) -> ArqBounds {
        self.agent_info.arq.to_bounds()
    }

    /// The ArqBounds to use when gossiping, as a singleton ArqSet
    pub fn arq_set(&self) -> ArqSet {
        ArqSet::single(self.arq_bounds())
    }

    /// Get the RegionSet for this node, suitable for gossiping
    pub fn region_set(&self, arq_set: ArqSet, now: Timestamp) -> RegionSet {
        let coords = RegionCoordSetXtcs::new(now, arq_set);
        let data = coords
            .region_coords_nested(self.tree.topo())
            .map(|columns| {
                columns
                    .map(|(_, coords)| self.query_region_data(&coords.to_bounds()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        RegionSetXtcs { coords, data }.into()
    }

    /// Quick 'n dirty simulation of a gossip round. Mutates both nodes as if
    /// they were exchanging gossip messages, without the rigmarole of a real protocol
    pub fn gossip_with(&mut self, other: &mut Self, now: Timestamp) -> TestNodeGossipRoundStats {
        let mut stats = TestNodeGossipRoundStats::default();

        assert_eq!(self.tree.topo(), other.tree.topo());
        let topo = self.tree.topo();

        // 1. calculate common arqset
        let common_arqs = self.arq_set().intersection(&other.arq_set());

        // 2. calculate and "send" regions
        let regions_self = self.region_set(common_arqs.clone(), now);
        let regions_other = other.region_set(common_arqs.clone(), now);
        stats.region_data_sent += regions_self.count() as u32 * Region::MASS;
        stats.region_data_rcvd += regions_other.count() as u32 * Region::MASS;

        // 3. calculate diffs and fetch ops
        let diff_self = regions_self.diff(&regions_other);
        let ops_self: Vec<_> = diff_self
            .region_coords(topo)
            .flat_map(|coords| self.query_op_data(&coords.to_bounds()))
            .collect();

        let diff_other = regions_other.diff(&regions_self);
        let ops_other: Vec<_> = diff_other
            .region_coords(topo)
            .flat_map(|coords| other.query_op_data(&coords.to_bounds()))
            .collect();

        // 4. "send" missing ops
        for op in ops_other {
            stats.op_data_rcvd += op.size;
            self.integrate_op(op);
        }
        for op in ops_self {
            stats.op_data_sent += op.size;
            other.integrate_op(op);
        }
        stats
    }
}

#[derive(Clone, Debug, Default)]
pub struct TestNodeGossipRoundStats {
    pub region_data_sent: u32,
    pub region_data_rcvd: u32,
    pub op_data_sent: u32,
    pub op_data_rcvd: u32,
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
