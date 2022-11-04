//! A Region is a bounded section of spacetime containing zero or more Ops.
//!
//! It consists of a [`RegionCoords`] object which defines the space and time
//! boundaries of the region, and some [`RegionData`] which contains a summary
//! of what is inside that region, including:
//! - the number of ops
//! - the total size of op data
//! - the XOR of all OpHashes within this region.
//!
//! The actual [`Region`] struct is generic over the data type, but in all
//! cases, we simply use RegionData, the default. (The type is generic for
//! the possibility of simpler testing in the future.)
//!
//! RegionData is composable: The sum of the RegionData of two *disjoint* (nonoverlapping)
//! Regions represents the union of those two Regions. The sum of hashes is defined
//! as the XOR of hashes, which allows this compatibility.

mod region_coords;
mod region_data;

pub use region_coords::*;
pub use region_data::*;

use num_traits::Zero;
use std::ops::{AddAssign, Sub};

/// The constant size in bytes of a region, used for calculating bandwidth usage during
/// gossip. All regions require the same number of bytes.
pub const REGION_MASS: u32 = std::mem::size_of::<Region<RegionData>>() as u32;

/// The coordinates defining the Region, along with the calculated [`RegionData`]
#[derive(Debug, Clone, derive_more::Constructor)]
pub struct Region<D: RegionDataConstraints = RegionData> {
    /// The coords
    pub coords: RegionCoords,
    /// The data
    pub data: D,
}

impl<D: RegionDataConstraints> Region<D> {}

/// The constraints necessary for any RegionData
pub trait RegionDataConstraints:
    Eq
    + Zero
    + AddAssign
    + Sub<Output = Self>
    + Clone
    + Send
    + Sync
    + std::fmt::Debug
    + serde::Serialize
    + serde::de::DeserializeOwned
{
    /// The number of ops in this region
    fn count(&self) -> u32;

    /// The size of all ops in this region
    fn size(&self) -> u32;

    // TODO: hash (not currently needed to be generic)
}
