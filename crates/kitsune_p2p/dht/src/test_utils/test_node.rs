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
    store: OpStore,
}

impl TestNode {
    pub fn new(topo: Topology, arq: Arq) -> Self {
        Self {
            agent: AgentKey(fake_hash()),
            agent_info: AgentInfo { arq },
            store: OpStore::new(topo),
        }
    }

    pub fn topo(&self) -> &Topology {
        self.store.tree.topo()
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
            .region_coords_nested(self.topo())
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

        assert_eq!(self.topo(), other.topo());
        let topo = self.topo();

        // 1. calculate common arqset
        let common_arqs = self.arq_set().intersection(&other.arq_set());

        // 2. calculate and "send" regions
        let regions_self = self.region_set(common_arqs.clone(), now);
        let regions_other = other.region_set(common_arqs.clone(), now);
        stats.region_data_sent += regions_self.count() as u32 * REGION_MASS;
        stats.region_data_rcvd += regions_other.count() as u32 * REGION_MASS;

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
        self.store.query_op_data(region)
    }

    fn query_region_data(&self, region: &RegionBounds) -> RegionData {
        self.store.query_region_data(region)
    }

    fn integrate_ops<Ops: Clone + Iterator<Item = Op>>(&mut self, ops: Ops) {
        self.store.integrate_ops(ops)
    }
}

impl AccessPeerStore for TestNode {
    fn get_agent_info(&self, _agent: AgentKey) -> crate::agent::AgentInfo {
        self.agent_info.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::op::OpData;

    use super::*;

    #[test]
    fn integrate_and_query_ops() {
        let topo = Topology::identity(Timestamp::from_micros(0));
        let arq = Arq::new(0.into(), 8, 4);
        let mut node = TestNode::new(topo, arq);

        node.integrate_ops(
            [
                OpData::fake(0, 10, 1234),
                OpData::fake(1000, 20, 2345),
                OpData::fake(2000, 15, 3456),
            ]
            .into_iter(),
        );
        {
            let data = node.query_region_data(&RegionBounds {
                x: (0.into(), 100.into()),
                t: (0.into(), 20.into()),
            });
            assert_eq!(data.count, 1);
            assert_eq!(data.size, 1234);
        }
        {
            let data = node.query_region_data(&RegionBounds {
                x: (0.into(), 1001.into()),
                t: (0.into(), 21.into()),
            });
            assert_eq!(data.count, 2);
            assert_eq!(data.size, 1234 + 2345);
        }
        {
            let data = node.query_region_data(&RegionBounds {
                x: (1000.into(), 1001.into()),
                t: (0.into(), 20.into()),
            });
            assert_eq!(data.count, 1);
            assert_eq!(data.size, 2345);
        }
    }

    #[test]
    fn gossip_regression() {
        let topo = Topology::identity(Timestamp::from_micros(0));
        let alice_arq = Arq::new(0.into(), 8, 4);
        let bobbo_arq = Arq::new(128.into(), 8, 4);
        let mut alice = TestNode::new(topo.clone(), alice_arq);
        let mut bobbo = TestNode::new(topo.clone(), bobbo_arq);

        alice.integrate_ops([OpData::fake(0, 10, 4321)].into_iter());
        bobbo.integrate_ops([OpData::fake(128, 20, 1234)].into_iter());

        // dbg!(&alice.tree.tree);
        let b = (4294967295, 71);
        let a = (4294967040, 64);

        let ne = alice.store.tree.tree.prefix_sum(b);
        let sw = alice.store.tree.tree.prefix_sum(a);
        assert_eq!(ne, sw);
        // alice.tree.tree.query((4294967040, 64), (4294967295, 71));
    }
}
