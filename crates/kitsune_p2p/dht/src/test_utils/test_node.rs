use crate::{
    agent::AgentInfo,
    arq::{ascii::add_location_ascii, *},
    coords::{GossipParams, TelescopingTimes, TimeCoord, Topology},
    hash::{fake_hash, AgentKey},
    host::{AccessOpStore, AccessPeerStore},
    op::Op,
    region::*,
};

use super::op_store::OpStore;

pub struct TestNode {
    _agent: AgentKey,
    agent_info: AgentInfo,
    store: OpStore,
}

impl TestNode {
    pub fn new(topo: Topology, gopa: GossipParams, arq: Arq) -> Self {
        Self {
            _agent: AgentKey(fake_hash()),
            agent_info: AgentInfo { arq },
            store: OpStore::new(topo, gopa),
        }
    }

    /// The Arq to use when gossiping
    pub fn arq(&self) -> Arq {
        self.agent_info.arq
    }

    /// The ArqBounds to use when gossiping
    pub fn arq_bounds(&self) -> ArqBounds {
        self.agent_info.arq.to_bounds()
    }

    /// Get the RegionSet for this node, suitable for gossiping
    pub fn region_set(&self, arq_set: ArqBoundsSet, now: TimeCoord) -> RegionSet {
        let coords = RegionCoordSetXtcs::new(TelescopingTimes::new(now), arq_set);
        let data = coords
            .region_coords_nested()
            .map(|columns| {
                columns
                    .map(|(_, coords)| self.query_region_coords(&coords))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        RegionSetXtcs { coords, data }.into()
    }

    pub fn ascii_arq_and_ops(&self, i: usize, len: usize) -> String {
        let arq = self.arq();
        format!(
            "|{}| {}: {}/{} @ {}",
            add_location_ascii(
                arq.to_ascii(len),
                self.store.ops.iter().map(|o| o.loc).collect()
            ),
            i,
            arq.power(),
            arq.count(),
            arq.center()
        )
    }
}

impl AccessOpStore for TestNode {
    fn query_op_data(&self, region: &RegionBounds) -> Vec<Op> {
        self.store.query_op_data(region)
    }

    fn query_region(&self, region: &RegionBounds) -> RegionData {
        self.store.query_region(region)
    }

    fn integrate_ops<Ops: Clone + Iterator<Item = Op>>(&mut self, ops: Ops) {
        self.store.integrate_ops(ops)
    }

    fn topo(&self) -> &Topology {
        self.store.topo()
    }

    fn gossip_params(&self) -> GossipParams {
        self.store.gossip_params()
    }
}

impl AccessPeerStore for TestNode {
    fn get_agent_info(&self, _agent: AgentKey) -> crate::agent::AgentInfo {
        self.agent_info.clone()
    }

    fn get_arq_set(&self) -> ArqBoundsSet {
        ArqBoundsSet::single(self.arq_bounds())
    }
}

#[cfg(test)]
mod tests {
    use kitsune_p2p_timestamp::Timestamp;

    use crate::op::OpData;

    use super::*;

    #[test]
    fn integrate_and_query_ops() {
        let topo = Topology::identity_zero();
        let gopa = GossipParams::zero();
        let arq = Arq::new(0.into(), 8, 4);
        let mut node = TestNode::new(topo, gopa, arq);

        node.integrate_ops(
            [
                OpData::fake(0.into(), Timestamp::from_micros(10), 1234),
                OpData::fake(1000.into(), Timestamp::from_micros(20), 2345),
                OpData::fake(2000.into(), Timestamp::from_micros(15), 3456),
            ]
            .into_iter(),
        );
        {
            let data = node.query_region(&RegionBounds {
                x: (0.into(), 100.into()),
                t: (0.into(), 20.into()),
            });
            assert_eq!(data.count, 1);
            assert_eq!(data.size, 1234);
        }
        {
            let data = node.query_region(&RegionBounds {
                x: (0.into(), 1001.into()),
                t: (0.into(), 21.into()),
            });
            assert_eq!(data.count, 2);
            assert_eq!(data.size, 1234 + 2345);
        }
        {
            let data = node.query_region(&RegionBounds {
                x: (1000.into(), 1001.into()),
                t: (0.into(), 20.into()),
            });
            assert_eq!(data.count, 1);
            assert_eq!(data.size, 2345);
        }
    }

    #[test]
    fn gossip_regression() {
        let topo = Topology::identity_zero();
        let gopa = GossipParams::zero();
        let alice_arq = Arq::new(0.into(), 8, 4);
        let bobbo_arq = Arq::new(128.into(), 8, 4);
        let mut alice = TestNode::new(topo.clone(), gopa, alice_arq);
        let mut bobbo = TestNode::new(topo.clone(), gopa, bobbo_arq);

        alice.integrate_ops([OpData::fake(0.into(), Timestamp::from_micros(10), 4321)].into_iter());
        bobbo.integrate_ops(
            [OpData::fake(128.into(), Timestamp::from_micros(20), 1234)].into_iter(),
        );

        // dbg!(&alice.tree.tree);
        let b = (4294967295, 71);
        let a = (4294967040, 64);

        let ne = alice.store.tree.tree.prefix_sum(b);
        let sw = alice.store.tree.tree.prefix_sum(a);
        assert_eq!(ne, sw);
        // alice.tree.tree.query((4294967040, 64), (4294967295, 71));
    }
}
