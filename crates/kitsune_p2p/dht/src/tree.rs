use std::ops::{AddAssign, Sub};

use num_traits::Zero;
use sparse_fenwick::Fenwick2;

use crate::{coords::*, op::*, region::*};

pub trait TreeDataConstraints:
    Zero + AddAssign + Sub<Output = Self> + Copy + std::fmt::Debug
{
}
impl<T> TreeDataConstraints for T where
    T: Zero + AddAssign + Sub<Output = T> + Copy + std::fmt::Debug
{
}

pub struct Tree<T: TreeDataConstraints = RegionData> {
    pub(crate) tree: Fenwick2<T>,
    topo: Topology,
}

impl<T: TreeDataConstraints> Tree<T> {
    pub fn new(topo: Topology) -> Self {
        Self {
            // TODO: take topology into account to reduce max size
            // TODO: can use a smaller time dimension
            tree: Fenwick2::new((SpaceCoord::MAX as usize + 1, TimeCoord::MAX as usize + 1)),
            topo,
        }
    }

    pub fn lookup(&self, region: &RegionBounds) -> T {
        let (sa, sb) = region.x;
        let (ta, tb) = region.t;
        self.tree.query((*sa, *ta), (*sb, *tb))
    }

    /// Get a reference to the tree's topo.
    pub fn topo(&self) -> &Topology {
        &self.topo
    }

    pub fn add(&mut self, (coords, data): (SpacetimeCoords, T)) {
        self.tree.update(coords.to_tuple(), data)
    }
}

impl Tree<RegionData> {
    pub fn add_op(&mut self, op: Op) {
        self.add(op.region_tuple(&self.topo));
    }
}
