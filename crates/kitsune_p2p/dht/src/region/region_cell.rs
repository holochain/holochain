use std::{
    iter::Sum,
    ops::{Add, AddAssign},
};

use num_traits::Zero;

use super::{RegionData, RegionDataConstraints};

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "test_utils", derive(Clone))]
/// Different states of region. If Locked, there is no need to include the data.
pub enum RegionCell<D = RegionData> {
    /// The data is present
    Data(D),
    /// The data is omitted because this region is locked.
    Locked,
}

impl<D: RegionDataConstraints> RegionCell<D> {
    /// Return data if not locked, else None
    pub fn as_option(&self) -> Option<&D> {
        match self {
            Self::Data(d) => Some(d),
            Self::Locked => None,
        }
    }

    /// Return data if not locked, else None
    pub fn into_option(self) -> Option<D> {
        match self {
            Self::Data(d) => Some(d),
            Self::Locked => None,
        }
    }

    /// This region is locked
    pub fn is_locked(&self) -> bool {
        *self == Self::Locked
    }

    /// Get the op count for this region, or zero if locked
    pub fn count(&self) -> u32 {
        match self {
            Self::Data(d) => d.count(),
            _ => 0,
        }
    }

    /// Get the size of ops in this region, or zero if locked
    pub fn size(&self) -> u32 {
        match self {
            Self::Data(d) => d.size(),
            _ => 0,
        }
    }
}

impl<D: AddAssign + Zero> Zero for RegionCell<D> {
    fn zero() -> Self {
        Self::Data(Zero::zero())
    }

    fn is_zero(&self) -> bool {
        match self {
            Self::Data(d) => d.is_zero(),
            // TODO: should we consider locked data as zero?
            _ => false,
        }
    }
}

impl<D: AddAssign> AddAssign for RegionCell<D> {
    fn add_assign(&mut self, other: Self) {
        match (self, other) {
            (Self::Data(ref mut a), Self::Data(b)) => {
                *a += b;
            }
            // Locked regions "infect" the whole sum
            (s, _) => *s = Self::Locked,
        }
    }
}

impl<D: AddAssign> Add for RegionCell<D> {
    type Output = Self;

    fn add(mut self, other: Self) -> Self::Output {
        self += other;
        self
    }
}

impl<D: Zero + AddAssign> Sum for RegionCell<D> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|a, b| a + b).unwrap_or_else(Zero::zero)
    }
}
