use crate::{
    coords::{SpaceCoord, SpaceInterval, TimeCoord, TimeInterval},
    region_data::RegionData,
    tree::Tree,
};

#[derive(Debug, derive_more::Constructor)]
pub struct Region {
    pub coords: RegionCoords,
    pub data: RegionData,
}

impl Region {
    pub const MASS: u32 = std::mem::size_of::<Region>() as u32;

    pub fn split(self, tree: &Tree) -> Option<(Self, Self)> {
        let (c1, c2) = self.coords.halve()?;
        let d1 = tree.lookup(&c1.to_bounds());
        let d2 = tree.lookup(&c2.to_bounds());
        let r1 = Self {
            coords: c1,
            data: d1,
        };
        let r2 = Self {
            coords: c2,
            data: d2,
        };
        Some((r1, r2))
    }
}

#[derive(Copy, Clone, Debug, derive_more::Constructor)]
pub struct RegionCoords {
    pub space: SpaceInterval,
    pub time: TimeInterval,
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

pub fn telescoping_times(now: TimeCoord) -> impl Iterator<Item = TimeInterval> {
    todo!();
    std::iter::empty()
}
