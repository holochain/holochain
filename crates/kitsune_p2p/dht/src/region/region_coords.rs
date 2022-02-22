use crate::Loc;
use kitsune_p2p_dht_arc::ArcInterval;
use kitsune_p2p_timestamp::Timestamp;

use crate::quantum::{SpaceSegment, SpacetimeCoords, TimeSegment, Topology};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, derive_more::Constructor)]
pub struct RegionCoords {
    pub space: SpaceSegment,
    pub time: TimeSegment,
}

impl RegionCoords {
    /// Map the quantized coordinates into the actual Timestamp and DhtLocation
    /// bounds specifying the region
    pub fn to_bounds(&self, topo: &Topology) -> RegionBounds {
        RegionBounds {
            x: self.space.loc_bounds(topo),
            t: self.time.timestamp_bounds(topo),
        }
    }

    pub fn contains(&self, coords: &SpacetimeCoords) -> bool {
        self.space.contains(coords.space) && self.time.contains(coords.time)
    }
}

#[derive(Debug)]
pub struct RegionBounds {
    pub x: (Loc, Loc),
    pub t: (Timestamp, Timestamp),
}

impl RegionBounds {
    pub fn new(x: (Loc, Loc), t: (Timestamp, Timestamp)) -> Self {
        Self { x, t }
    }

    pub fn arc_interval(&self) -> ArcInterval {
        ArcInterval::new(self.x.0, self.x.1)
    }

    pub fn time_range(&self) -> std::ops::RangeInclusive<Timestamp> {
        self.t.0..=self.t.1
    }
}
