use kitsune_p2p_dht_arc::ArcInterval;
use kitsune_p2p_timestamp::Timestamp;

use crate::quantum::{SpaceSegment, SpacetimeCoords, TimeSegment, Topology};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, derive_more::Constructor)]
pub struct RegionCoords {
    pub space: SpaceSegment,
    pub time: TimeSegment,
}

impl RegionCoords {
    /// TODO: does this need to map to the actual absolute values, i.e. undergo
    /// topological transformation, or is this correct?
    pub fn to_bounds(&self, topo: &Topology) -> RegionBounds {
        let (x0, x1) = self.space.loc_bounds(topo);
        let (t0, t1) = self.time.timestamp_bounds(topo);
        RegionBounds {
            x: ArcInterval::new(x0, x1),
            t: t0..t1,
        }
    }

    pub fn contains(&self, coords: &SpacetimeCoords) -> bool {
        self.space.contains(coords.space) && self.time.contains(coords.time)
    }
}

#[derive(Debug)]
pub struct RegionBounds {
    pub x: ArcInterval,
    pub t: std::ops::Range<Timestamp>,
}
