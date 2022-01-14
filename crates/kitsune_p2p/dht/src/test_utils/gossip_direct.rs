use kitsune_p2p_timestamp::Timestamp;

use crate::{host::HostAccess, region::*};

pub fn gossip_direct_at<Peer: HostAccess>(
    left: &mut Peer,
    right: &mut Peer,
    now: Timestamp,
) -> TestNodeGossipRoundStats {
    gossip_direct((left, now), (right, now))
}

/// Quick 'n dirty simulation of a gossip round. Mutates both nodes as if
/// they were exchanging gossip messages, without the rigmarole of a real protocol
pub fn gossip_direct<Peer: HostAccess>(
    (left, time_left): (&mut Peer, Timestamp),
    (right, time_right): (&mut Peer, Timestamp),
) -> TestNodeGossipRoundStats {
    let mut stats = TestNodeGossipRoundStats::default();

    assert_eq!(left.topo(), right.topo());
    let topo = left.topo();
    let common_arqs = {
        // ROUND I: Initial handshake, exchange ArqSets

        // - calculate common arqset
        left.get_arq_set().intersection(&right.get_arq_set())
    };

    {
        // ROUND II: Send Agents (not shown)
    }

    let (regions_left, regions_right) = {
        // ROUND III: Calculate and send Region data

        // - calculate regions
        let regions_left = left.region_set(common_arqs.clone(), time_left);
        let regions_right = right.region_set(common_arqs.clone(), time_right);
        stats.region_data_sent += regions_left.count() as u32 * REGION_MASS;
        stats.region_data_rcvd += regions_right.count() as u32 * REGION_MASS;
        (regions_left, regions_right)
    };

    {
        // ROUND IV: Calculate diffs and send missing ops

        // - calculate diffs
        let diff_left = regions_left.diff(&regions_right);
        let diff_right = regions_right.diff(&regions_left);

        // - fetch ops
        let ops_left: Vec<_> = diff_left
            .region_coords(topo)
            .flat_map(|coords| left.query_op_data(&coords.to_bounds()))
            .collect();
        let ops_right: Vec<_> = diff_right
            .region_coords(topo)
            .flat_map(|coords| right.query_op_data(&coords.to_bounds()))
            .collect();

        // - "send" missing ops
        for op in ops_right {
            stats.op_data_rcvd += op.size;
            left.integrate_op(op);
        }
        for op in ops_left {
            stats.op_data_sent += op.size;
            right.integrate_op(op);
        }
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
