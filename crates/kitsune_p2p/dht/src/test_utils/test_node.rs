use must_future::MustBoxFuture;

use crate::{
    arq::{ascii::add_location_ascii, *},
    hash::{fake_hash, AgentKey},
    persistence::{AccessOpStore, AccessPeerStore},
    prelude::RegionCoordSetLtcs,
    region::*,
    region_set::*,
    spacetime::{GossipParams, TelescopingTimes, TimeQuantum, Topology},
};

use super::{
    op_data::{Op, OpData},
    op_store::OpStore,
};

/// A "node", with test-worthy implementation of the host interface
pub struct TestNode {
    _agent: AgentKey,
    agent_arq: Arq,
    store: OpStore,
}

impl TestNode {
    /// Constructor
    pub fn new(topo: Topology, gopa: GossipParams, arq: Arq) -> Self {
        Self {
            _agent: AgentKey(fake_hash()),
            agent_arq: arq,
            store: OpStore::new(topo, gopa),
        }
    }

    /// The Arq to use when gossiping
    pub fn arq(&self) -> Arq {
        self.agent_arq
    }

    /// The ArqBounds to use when gossiping
    pub fn arq_bounds(&self) -> ArqBounds {
        self.agent_arq.to_bounds(self.topo())
    }

    /// Get the RegionSet for this node, suitable for gossiping
    pub fn region_set(&self, arq_set: ArqBoundsSet, now: TimeQuantum) -> RegionSet {
        let coords = RegionCoordSetLtcs::new(TelescopingTimes::new(now), arq_set);
        let data = coords
            .region_coords_nested()
            .map(|columns| {
                columns
                    .map(|(_, coords)| self.query_region_data(&coords))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        RegionSetLtcs::from_data(coords, data).into()
    }

    /// Print an ascii representation of the node's arq and all ops held
    pub fn ascii_arq_and_ops(&self, topo: &Topology, i: usize, len: usize) -> String {
        let arq = self.arq();
        format!(
            "|{}| {}: {}/{} @ {}",
            add_location_ascii(
                arq.to_ascii(topo, len),
                self.store.ops.iter().map(|o| o.loc).collect()
            ),
            i,
            arq.power(),
            arq.count(),
            arq.start_loc()
        )
    }
}

impl AccessOpStore<OpData> for TestNode {
    fn query_op_data(&self, region: &RegionCoords) -> Vec<Op> {
        self.store.query_op_data(region)
    }

    fn query_region_data(&self, region: &RegionCoords) -> RegionData {
        self.store.query_region_data(region)
    }

    fn fetch_region_set(
        &self,
        coords: RegionCoordSetLtcs,
    ) -> MustBoxFuture<Result<RegionSetLtcs, ()>> {
        self.store.fetch_region_set(coords)
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
    fn get_agent_arq(&self, _agent: AgentKey) -> crate::arq::Arq {
        self.agent_arq.clone()
    }

    fn get_arq_set(&self) -> ArqBoundsSet {
        ArqBoundsSet::single(self.arq_bounds())
    }
}

#[cfg(test)]
mod tests {
    use kitsune_p2p_timestamp::Timestamp;
    use std::str::FromStr;

    use crate::spacetime::*;

    use super::*;

    #[test]
    fn integrate_and_query_ops() {
        let topo = Topology::unit_zero();
        let gopa = GossipParams::zero();
        let arq = Arq::new(8, 0u32.into(), 4.into());
        let mut node = TestNode::new(topo.clone(), gopa, arq);

        node.integrate_ops(
            [
                OpData::fake(0u32.into(), Timestamp::from_micros(10), 1234),
                OpData::fake(1000u32.into(), Timestamp::from_micros(20), 2345),
                OpData::fake(2000u32.into(), Timestamp::from_micros(15), 3456),
            ]
            .into_iter(),
        );
        {
            let coords = RegionCoords {
                space: SpaceSegment::new(7, 0),
                time: TimeSegment::new(5, 0),
            };
            dbg!(coords.to_bounds(&topo));
            let data = node.query_region_data(&coords);
            assert_eq!(data.count, 1);
            assert_eq!(data.size, 1234);
        }
        {
            let coords = RegionCoords {
                space: SpaceSegment::new(10, 0),
                time: TimeSegment::new(5, 0),
            };
            dbg!(coords.to_bounds(&topo));
            let data = node.query_region_data(&coords);
            assert_eq!(data.count, 2);
            assert_eq!(data.size, 1234 + 2345);
        }
        {
            let coords = RegionCoords {
                space: SpaceSegment::new(10, 1),
                time: TimeSegment::new(5, 0),
            };
            dbg!(coords.to_bounds(&topo));
            let data = node.query_region_data(&coords);
            assert_eq!(data.count, 1);
            assert_eq!(data.size, 3456);
        }
    }

    #[test]
    fn integrate_and_query_ops_standard_topo() {
        let topo = Topology::standard_epoch();
        let gopa = GossipParams::zero();
        let arq = Arq::new(8, 0u32.into(), 4.into());
        let mut node = TestNode::new(topo.clone(), gopa, arq);

        let p = pow2(12);

        node.integrate_ops(
            [
                OpData::fake(
                    // origin
                    1.into(),
                    // origin
                    Timestamp::from_str("2022-01-01T00:02:00Z").unwrap(),
                    1234,
                ),
                OpData::fake(
                    // 10 quanta from origin
                    (p * 10).into(),
                    // 1 quantum from origin
                    Timestamp::from_str("2022-01-01T00:05:00Z").unwrap(),
                    2345,
                ),
                OpData::fake(
                    (p * 100).into(),
                    // 12 * 24 quanta from origin
                    Timestamp::from_str("2022-01-02T00:00:00Z").unwrap(),
                    3456,
                ),
            ]
            .into_iter(),
        );
        {
            let coords = RegionCoords {
                space: SpaceSegment::new(0, 0),
                time: TimeSegment::new(0, 0),
            };
            dbg!(coords.to_bounds(&topo));
            let data = node.query_region_data(&coords);
            assert_eq!(data.count, 1);
            assert_eq!(data.size, 1234);
        }
        {
            let coords = RegionCoords {
                space: SpaceSegment::new(4, 0),
                time: TimeSegment::new(1, 0),
            };
            dbg!(coords.to_bounds(&topo));
            let data = node.query_region_data(&coords);
            assert_eq!(data.count, 2);
            assert_eq!(data.size, 1234 + 2345);
        }
        {
            let coords = RegionCoords {
                space: SpaceSegment::new(2, 25),
                time: TimeSegment::new(0, 12 * 24),
            };
            dbg!(coords.to_bounds(&topo));
            let data = node.query_region_data(&coords);
            assert_eq!(data.count, 1);
            assert_eq!(data.size, 3456);
        }
    }

    #[test]
    #[cfg(obsolete)]
    fn gossip_regression() {
        let topo = Topology::unit_zero();
        let gopa = GossipParams::zero();
        let alice_arq = Arq::new(0u32.into(), 8, 4);
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
