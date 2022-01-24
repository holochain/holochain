use crate::{
    coords::TimeCoord,
    error::{GossipError, GossipResult},
    host::HostAccess,
    region::*,
};

pub fn gossip_direct_at<Peer: HostAccess>(
    left: &mut Peer,
    right: &mut Peer,
    now: TimeCoord,
) -> GossipResult<TestNodeGossipRoundStats> {
    gossip_direct((left, now), (right, now))
}

/// Quick 'n dirty simulation of a gossip round. Mutates both nodes as if
/// they were exchanging gossip messages, without the rigmarole of a real protocol
pub fn gossip_direct<Peer: HostAccess>(
    (left, time_left): (&mut Peer, TimeCoord),
    (right, time_right): (&mut Peer, TimeCoord),
) -> GossipResult<TestNodeGossipRoundStats> {
    let mut stats = TestNodeGossipRoundStats::default();

    // - ensure identical topologies (especially the time_origin)
    if left.topo() != right.topo() {
        return Err(GossipError::TopologyMismatch);
    }
    let _topo = left.topo();

    let common_arqs = {
        // ROUND I: Initial handshake, exchange ArqSets and as-at timestamps

        let gpl = left.gossip_params();
        let gpr = right.gossip_params();

        // - ensure compatible as-at timestamps
        let tl = *time_left as i64;
        let tr = *time_right as i64;
        if (tl - tr).abs() as u32 > u32::min(*gpl.max_time_offset, *gpr.max_time_offset) {
            return Err(GossipError::TimesOutOfSync);
        }

        // - calculate common arqset
        let al = left.get_arq_set();
        let ar = right.get_arq_set();
        if (al.power() as i8 - ar.power() as i8).abs() as u8
            > u8::min(gpl.max_space_power_offset, gpr.max_space_power_offset)
        {
            return Err(GossipError::ArqPowerDiffTooLarge);
        }
        al.intersection(&ar)
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
        let diff_left = regions_left.clone().diff(regions_right.clone())?;
        let diff_right = regions_right.diff(regions_left)?;

        // - fetch ops
        let ops_left: Vec<_> = diff_left
            .iter()
            .flat_map(|r| left.query_ops_by_coords(&r.coords))
            .collect();
        let ops_right: Vec<_> = diff_right
            .iter()
            .flat_map(|r| right.query_ops_by_coords(&r.coords))
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
    Ok(stats)
}

#[derive(Clone, Debug, Default, derive_more::Add)]
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
