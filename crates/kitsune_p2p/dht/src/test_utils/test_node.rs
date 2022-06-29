use std::collections::HashMap;

use must_future::MustBoxFuture;

use crate::{
    arq::{ascii::add_location_ascii, *},
    hash::AgentKey,
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
    arqs: HashMap<AgentKey, Arq>,
    store: OpStore,
}

impl TestNode {
    /// Constructor
    pub fn new(topo: Topology, gopa: GossipParams, arqs: HashMap<AgentKey, Arq>) -> Self {
        Self {
            arqs,
            store: OpStore::new(topo, gopa),
        }
    }
    /// Constructor
    pub fn new_single(topo: Topology, gopa: GossipParams, arq: Arq) -> (Self, AgentKey) {
        let agent_key = AgentKey::fake();
        let node = Self::new(topo, gopa, [(agent_key.clone(), arq)].into_iter().collect());
        (node, agent_key)
    }

    /// Get the RegionSet for this node, suitable for gossiping
    pub fn region_set(&self, arq_set: ArqBoundsSet, now: TimeQuantum) -> RegionSet {
        let coords = RegionCoordSetLtcs::new(TelescopingTimes::new(now), arq_set);
        coords
            .into_region_set_infallible(|(_, coords)| self.query_region_data(&coords))
            .into()
    }

    /// Print an ascii representation of the node's arq and all ops held
    pub fn ascii_arqs_and_ops(&self, topo: &Topology, len: usize) -> String {
        self.arqs
            .iter()
            .enumerate()
            .map(|(i, (_, arq))| {
                format!(
                    "{:>3}: |{}| {}/{} @ {}\n",
                    i,
                    add_location_ascii(
                        arq.to_ascii(topo, len),
                        self.store.ops.iter().map(|o| o.loc).collect()
                    ),
                    arq.power(),
                    arq.count(),
                    arq.start_loc()
                )
            })
            .collect()
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
    fn get_agent_arq(&self, agent: &AgentKey) -> crate::arq::Arq {
        *self.arqs.get(agent).unwrap()
    }

    fn get_arq_set(&self) -> ArqBoundsSet {
        ArqBoundsSet::new(
            self.arqs
                .iter()
                .map(|(_, arq)| arq.to_bounds(&self.store.topo))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use kitsune_p2p_timestamp::Timestamp;

    use crate::spacetime::*;

    use super::*;

    #[test]
    fn integrate_and_query_ops() {
        let topo = Topology::unit_zero();
        let gopa = GossipParams::zero();
        let arq = Arq::new(8, 0u32.into(), 4.into());
        let (mut node, _) = TestNode::new_single(topo.clone(), gopa, arq);

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
        let (mut node, _) = TestNode::new_single(topo.clone(), gopa, arq);

        let p = pow2(12);

        node.integrate_ops(
            [
                OpData::fake(
                    // origin
                    1u32.into(),
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
}
