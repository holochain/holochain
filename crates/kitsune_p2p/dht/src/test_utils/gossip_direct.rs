use crate::{
    error::{GossipError, GossipResult},
    persistence::HostAccessTest,
    prelude::ArqBoundsSet,
    region::REGION_MASS,
    spacetime::{Quantum, TimeQuantum},
};

/// Do [`gossip_direct`], with both nodes at the same current time
pub fn gossip_direct_at<Peer: HostAccessTest>(
    left: &mut Peer,
    right: &mut Peer,
    now: TimeQuantum,
) -> GossipResult<TestNodeGossipRoundInfo> {
    gossip_direct((left, now), (right, now))
}

/// Quick 'n dirty simulation of a gossip round. Mutates both nodes as if
/// they were exchanging gossip messages, without the rigmarole of a real protocol
pub fn gossip_direct<Peer: HostAccessTest>(
    (left, time_left): (&mut Peer, TimeQuantum),
    (right, time_right): (&mut Peer, TimeQuantum),
) -> GossipResult<TestNodeGossipRoundInfo> {
    let mut stats = TestNodeGossipRoundStats::default();

    // - ensure identical topologies (especially the time_origin)
    if left.topo() != right.topo() {
        return Err(GossipError::TopologyMismatch);
    }
    let topo = left.topo();

    let common_arqs = {
        // ROUND I: Initial handshake, exchange ArqSets and as-at timestamps

        let gpl = left.gossip_params();
        let gpr = right.gossip_params();

        // - ensure compatible as-at timestamps
        let tl = time_left.inner() as i64;
        let tr = time_right.inner() as i64;
        if (tl - tr).unsigned_abs() as u32
            > u32::min(gpl.max_time_offset.inner(), gpr.max_time_offset.inner())
        {
            return Err(GossipError::TimesOutOfSync);
        }

        // - calculate common arqset
        let al = left.get_arq_set();
        let ar = right.get_arq_set();
        al.print_arqs(topo, 64);
        ar.print_arqs(topo, 64);
        if (al.power() as i8 - ar.power() as i8).unsigned_abs()
            > u8::min(gpl.max_space_power_offset, gpr.max_space_power_offset)
        {
            return Err(GossipError::ArqPowerDiffTooLarge);
        }
        al.intersection(topo, &ar)
    };

    {
        // ROUND II: Send Agents (not shown)
    }

    let (regions_left, regions_right) = {
        // ROUND III: Calculate and send Region data

        // - calculate regions
        let regions_left = left.region_set(common_arqs.clone(), time_left);
        let regions_right = right.region_set(common_arqs.clone(), time_right);
        stats.regions_sent += regions_left.count() as u32;
        stats.regions_rcvd += regions_right.count() as u32;
        (regions_left, regions_right)
    };
    {
        // ROUND IV: Calculate diffs and send missing ops

        // - calculate diffs
        let diff_left = regions_left.clone().diff(regions_right.clone())?;
        let diff_right = regions_right.diff(regions_left)?;

        // - fetch ops
        let ops_left: Vec<_> = diff_left
            .iter()
            .flat_map(|r| left.query_op_data(&r.coords))
            .collect();
        let ops_right: Vec<_> = diff_right
            .iter()
            .flat_map(|r| right.query_op_data(&r.coords))
            .collect();

        // - "send" missing ops
        for op in ops_right {
            stats.ops_rcvd += 1;
            stats.op_data_rcvd += op.size as u64;
            left.integrate_op(op);
        }
        for op in ops_left {
            stats.ops_sent += 1;
            stats.op_data_sent += op.size as u64;
            right.integrate_op(op);
        }
    }
    Ok(TestNodeGossipRoundInfo { common_arqs, stats })
}

/// Useful data calculated during the test node gossip round
pub struct TestNodeGossipRoundInfo {
    /// The common arq set calculated during gossip
    pub common_arqs: ArqBoundsSet,
    /// Stats about data transfer during the round
    pub stats: TestNodeGossipRoundStats,
}

/// Stats about what was sent and received during the gossip round
#[derive(Clone, Debug, Default, derive_more::Add)]
#[allow(missing_docs)]
pub struct TestNodeGossipRoundStats {
    pub regions_sent: u32,
    pub regions_rcvd: u32,
    pub ops_sent: u32,
    pub ops_rcvd: u32,
    pub op_data_sent: u64,
    pub op_data_rcvd: u64,
}

impl TestNodeGossipRoundStats {
    /// The total bytes sent
    pub fn total_sent(&self) -> u64 {
        (self.regions_sent * REGION_MASS) as u64 + self.op_data_sent
    }

    /// The total bytes received
    pub fn total_rcvd(&self) -> u64 {
        (self.regions_rcvd * REGION_MASS) as u64 + self.op_data_rcvd
    }
}
