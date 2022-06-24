use crate::Loc;
use kitsune_p2p_dht_arc::DhtArc;
use kitsune_p2p_timestamp::Timestamp;

use crate::spacetime::{SpaceSegment, SpacetimeQuantumCoords, TimeSegment, Topology};

/// The cross product of a space segment and at time segment forms a Region.
/// Hence, these two segments are the coordinates which define a Region of spacetime.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, derive_more::Constructor)]
pub struct RegionCoords {
    /// The space segment
    pub space: SpaceSegment,
    /// The time segment
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

    /// Does the region contain this spacetime quantum?
    pub fn contains(&self, topo: &Topology, coords: &SpacetimeQuantumCoords) -> bool {
        self.space.contains_quantum(topo, coords.space)
            && self.time.contains_quantum(topo, coords.time)
    }

    /// Split this region into 4 equal subregions, if possible.
    /// If one dimension is quantum, bisect the non-quantum dimension.
    /// If both dimensions are quantum, return None.
    pub fn quadrisect(self) -> Option<Vec<Self>> {
        let Self { space, time } = self;
        let (ss, ts) = match (space.bisect(), time.bisect()) {
            (Some(ss), Some(ts)) => Some((ss.to_vec(), ts.to_vec())),
            (Some(ss), None) => Some((ss.to_vec(), vec![time])),
            (None, Some(ts)) => Some((vec![space], ts.to_vec())),
            (None, None) => None,
        }?;
        Some(
            ss.into_iter()
                .flat_map(|space| ts.iter().map(move |&time| RegionCoords { space, time }))
                .collect(),
        )
    }
}

/// A region specified in absolute coords, rather than quantum coords.
/// This type should only be used in the host, which deals in absolute coords.
/// Kitsune itself should only use [`RegionCoords`] to ensure proper quantum
/// alignment.
#[derive(Debug)]
pub struct RegionBounds {
    /// The inclusive min and max locations
    pub x: (Loc, Loc),
    /// The inclusive min and max timestamps
    pub t: (Timestamp, Timestamp),
}

impl RegionBounds {
    /// Constructor from extrema
    pub fn new<L: Into<Loc>>((a, b): (L, L), t: (Timestamp, Timestamp)) -> Self {
        Self {
            x: (a.into(), b.into()),
            t,
        }
    }

    /// Does this region contain this point?
    pub fn contains(&self, x: &Loc, t: &Timestamp) -> bool {
        self.arc_interval().contains(x) && self.time_range().contains(t)
    }

    /// Just the primitive underlying numbers. For diagnostics.
    pub fn to_primitive(&self) -> ((u32, u32), (i64, i64)) {
        (
            (self.x.0.as_u32(), self.x.1.as_u32()),
            (self.t.0.as_micros(), self.t.1.as_micros()),
        )
    }

    fn arc_interval(&self) -> DhtArc {
        DhtArc::from_bounds(self.x.0, self.x.1)
    }

    fn time_range(&self) -> std::ops::RangeInclusive<Timestamp> {
        self.t.0..=self.t.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_bounds_regressions() {
        use std::str::FromStr;
        let topo = Topology::standard_epoch_full();
        let b =
            RegionCoords::new(SpaceSegment::new(12, 100), TimeSegment::new(4, 12)).to_bounds(&topo);

        dbg!(&b);
        assert_eq!(b.x.0, 1677721600.into());
        assert_eq!(b.x.1, 1694498815.into());
        assert_eq!(b.t.0, Timestamp::from_str("2022-01-01T16:00:00Z").unwrap());
        assert_eq!(
            b.t.1,
            Timestamp::from_str("2022-01-01T17:19:59.999999Z").unwrap()
        );
    }

    #[test]
    fn test_quadrisect() {
        assert_eq!(
            RegionCoords::new(SpaceSegment::new(12, 100), TimeSegment::new(4, 12)).quadrisect(),
            Some(vec![
                RegionCoords::new(SpaceSegment::new(11, 200), TimeSegment::new(3, 24)),
                RegionCoords::new(SpaceSegment::new(11, 200), TimeSegment::new(3, 25)),
                RegionCoords::new(SpaceSegment::new(11, 201), TimeSegment::new(3, 24)),
                RegionCoords::new(SpaceSegment::new(11, 201), TimeSegment::new(3, 25)),
            ])
        );
        assert_eq!(
            RegionCoords::new(SpaceSegment::new(12, 100), TimeSegment::new(0, 12)).quadrisect(),
            Some(vec![
                RegionCoords::new(SpaceSegment::new(11, 200), TimeSegment::new(0, 12)),
                RegionCoords::new(SpaceSegment::new(11, 201), TimeSegment::new(0, 12)),
            ])
        );
        assert_eq!(
            RegionCoords::new(SpaceSegment::new(0, 100), TimeSegment::new(4, 12)).quadrisect(),
            Some(vec![
                RegionCoords::new(SpaceSegment::new(0, 100), TimeSegment::new(3, 24)),
                RegionCoords::new(SpaceSegment::new(0, 100), TimeSegment::new(3, 25)),
            ])
        );
        assert_eq!(
            RegionCoords::new(SpaceSegment::new(0, 100), TimeSegment::new(0, 12)).quadrisect(),
            None
        );
    }
}
