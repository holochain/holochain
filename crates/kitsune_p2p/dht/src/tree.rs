use std::ops::{AddAssign, Sub};

use num_traits::Zero;
use sparse_fenwick::Fenwick2;

use crate::{coords::*, op::Op, region::*};

pub trait TreeDataConstraints:
    Zero + AddAssign + Sub<Output = Self> + Copy + std::fmt::Debug
{
}
impl<T> TreeDataConstraints for T where
    T: Zero + AddAssign + Sub<Output = T> + Copy + std::fmt::Debug
{
}

pub struct TreeImpl<T: TreeDataConstraints> {
    pub(crate) tree: Fenwick2<T>,
    topo: Topology,
}

impl<T: TreeDataConstraints> TreeImpl<T> {
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
}

impl TreeImpl<RegionData> {
    pub fn add_op(&mut self, op: Op) {
        let (coords, data) = op.to_tree_data(&self.topo);
        self.tree.update(coords.to_tuple(), data);
    }
}

pub type Tree = TreeImpl<RegionData>;
