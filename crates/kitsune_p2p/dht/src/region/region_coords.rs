use crate::coords::{SpaceCoord, SpaceSegment, TimeCoord, TimeSegment};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, derive_more::Constructor)]
pub struct RegionCoords {
    pub space: SpaceSegment,
    pub time: TimeSegment,
}

impl RegionCoords {
    pub fn halve(self) -> Option<(Self, Self)> {
        let (sa, sb) = self.space.halve()?;
        Some((
            Self {
                space: sa,
                time: self.time,
            },
            Self {
                space: sb,
                time: self.time,
            },
        ))
    }

    pub fn to_bounds(&self) -> RegionBounds {
        RegionBounds {
            x: self.space.bounds(),
            t: self.time.bounds(),
        }
    }
}

pub struct RegionBounds {
    pub x: (SpaceCoord, SpaceCoord),
    pub t: (TimeCoord, TimeCoord),
}
