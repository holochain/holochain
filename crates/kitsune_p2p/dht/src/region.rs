use crate::{coords::RegionCoords, region_data::RegionData, tree::Tree};

pub struct Region {
    pub coords: RegionCoords,
    pub data: RegionData,
}

impl Region {
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
