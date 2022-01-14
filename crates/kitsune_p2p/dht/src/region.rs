mod region_coords;
mod region_data;
mod region_set;

pub use region_coords::*;
pub use region_data::*;
pub use region_set::*;

use crate::tree::*;

pub const REGION_MASS: u32 = std::mem::size_of::<Region<RegionData>>() as u32;

#[derive(Debug, derive_more::Constructor)]
pub struct Region<T: TreeDataConstraints = RegionData> {
    pub coords: RegionCoords,
    pub data: T,
}

impl<T: TreeDataConstraints> Region<T> {
    pub fn split(self, tree: &Tree<T>) -> Option<(Self, Self)> {
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
