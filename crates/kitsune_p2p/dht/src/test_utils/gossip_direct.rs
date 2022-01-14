use kitsune_p2p_timestamp::Timestamp;

use crate::{host::HostAccess, region::*};

/// Quick 'n dirty simulation of a gossip round. Mutates both nodes as if
/// they were exchanging gossip messages, without the rigmarole of a real protocol
pub fn gossip_direct<Peer: HostAccess>(
    left: &mut Peer,
    right: &mut Peer,
    now: Timestamp,
) -> TestNodeGossipRoundStats {
    let mut stats = TestNodeGossipRoundStats::default();

    assert_eq!(left.topo(), right.topo());
    let topo = left.topo();

    // 1. calculate common arqset
    let common_arqs = left.get_arq_set().intersection(&right.get_arq_set());

    // 2. calculate and "send" regions
    let regions_left = left.region_set(common_arqs.clone(), now);
    let regions_right = right.region_set(common_arqs.clone(), now);
    stats.region_data_sent += regions_left.count() as u32 * REGION_MASS;
    stats.region_data_rcvd += regions_right.count() as u32 * REGION_MASS;

    // 3. calculate diffs and fetch ops
    let diff_left = regions_left.diff(&regions_right);
    let ops_left: Vec<_> = diff_left
        .region_coords(topo)
        .flat_map(|coords| left.query_op_data(&coords.to_bounds()))
        .collect();

    let diff_right = regions_right.diff(&regions_left);
    let ops_right: Vec<_> = diff_right
        .region_coords(topo)
        .flat_map(|coords| right.query_op_data(&coords.to_bounds()))
        .collect();

    // 4. "send" missing ops
    for op in ops_right {
        stats.op_data_rcvd += op.size;
        left.integrate_op(op);
    }
    for op in ops_left {
        stats.op_data_sent += op.size;
        right.integrate_op(op);
    }
    stats
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
