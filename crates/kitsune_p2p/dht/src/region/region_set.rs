use std::collections::HashMap;

use crate::tree::TreeDataConstraints;

use super::{RegionCoords, RegionData, RegionImpl};

pub struct RegionSetImpl<T: TreeDataConstraints>(HashMap<RegionCoords, T>);

impl<T: TreeDataConstraints> RegionSetImpl<T> {
    pub fn new(regions: Vec<RegionImpl<T>>) -> Self {
        Self(regions.into_iter().map(|r| (r.coords, r.data)).collect())
    }

    /// Find a set of Regions which represents the intersection of the two
    /// input RegionSets.
    ///
    /// If the Regions don't line up, then some regions will be combined in an
    /// attempt to match a Region in the other set
    pub fn diff(&self, other: &Self) -> Vec<RegionImpl<T>> {
        // can we use a Fenwick tree to look up regions?
        // idea:
        // sort the regions by power (problem, there are two power)
        // lookup the region to see if there's already a direct hit (most efficient if the sorting guarantees that larger regions get looked up later)
        // PROBLEM: we *can't* resolve rectangles where one is not a subset of the other
        //
        todo!()
    }
}

pub type RegionSet = RegionSetImpl<RegionData>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_diff() {
        todo!()
        // let coords = [
        //     RegionCoords::new(space, time)
        // ]
        // let a = RegionSetImpl::new()
    }
}
