mod error;
mod region_coords;
mod region_data;
mod region_set;

pub use error::*;
pub use region_coords::*;
pub use region_data::*;
pub use region_set::*;

use crate::{
    coords::{SpacetimeCoords, Topology},
    op::{Op, OpRegion},
    tree::*,
};

pub const REGION_MASS: u32 = std::mem::size_of::<Region<RegionData>>() as u32;

#[derive(Debug, derive_more::Constructor)]
pub struct Region<D: TreeDataConstraints = RegionData> {
    pub coords: RegionCoords,
    pub data: D,
}

impl<D: TreeDataConstraints> Region<D> {}
