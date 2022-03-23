use std::ops::{AddAssign, Sub};

use num_traits::Zero;

use crate::{op::*, quantum::*, region::*, test_utils::op_data::Op};

pub trait TreeDataConstraints:
    Eq
    + Zero
    + AddAssign
    + Sub<Output = Self>
    + Copy
    + Send
    + Sync
    + std::fmt::Debug
    + serde::Serialize
    + serde::de::DeserializeOwned
{
}
impl<T> TreeDataConstraints for T where
    T: Eq
        + Zero
        + AddAssign
        + Sub<Output = T>
        + Copy
        + Send
        + Sync
        + std::fmt::Debug
        + serde::Serialize
        + serde::de::DeserializeOwned
{
}

#[derive(Clone)]
pub struct Tree<D: TreeDataConstraints = RegionData> {
    pub(crate) tree: RegionSet<D>,
    topo: Topology,
}

impl<D: TreeDataConstraints> Tree<D> {
    pub fn new(topo: Topology, region_set: RegionSet<D>) -> Self {
        Self {
            // TODO: take topology into account to reduce max size
            // TODO: can use a smaller time dimension
            tree: region_set,
            topo,
        }
    }

    pub fn lookup(&self, region: &RegionBounds) -> D {
        self.tree.query(region)
    }

    /// Get a reference to the tree's topo.
    pub fn topo(&self) -> &Topology {
        &self.topo
    }

    pub fn add(&mut self, (coords, data): (SpacetimeCoords, D)) {
        self.tree.update(coords, data)
    }
}

impl Tree<RegionData> {
    pub fn add_op(&mut self, op: Op) {
        self.add((op.coords(self.topo()), op.region_data()));
    }
}
