use std::collections::HashMap;

use kitsune_p2p_dht_arc::ArcInterval;
use kitsune_p2p_timestamp::Timestamp;

use crate::{arq::*, coords::*, tree::TreeDataConstraints};

use super::{RegionCoords, RegionData, RegionImpl};

#[derive(Debug, derive_more::Constructor)]
pub struct RegionCoordSetXtcs {
    max_time: Timestamp,
    arq: ArqBounds,
}

impl RegionCoordSetXtcs {
    /// Generate the XTCS region coords given the generating parameters.
    /// Each RegionCoords is paired with the relative spacetime coords, which
    /// can be used to pair the generated coords with stored data.
    pub fn region_coords<'a>(
        &'a self,
        topo: &'a Topology,
    ) -> impl Iterator<Item = ((SpaceCoord, TimeCoord), RegionCoords)> + 'a {
        self.arq.segments().enumerate().flat_map(move |(ix, x)| {
            topo.telescoping_times(self.max_time)
                .into_iter()
                .enumerate()
                .map(move |(it, t)| {
                    (
                        (SpaceCoord::from(ix as u32), TimeCoord::from(it as u32)),
                        RegionCoords::new(x, t),
                    )
                })
        })
    }

    pub fn region_coords_nested<'a>(
        &'a self,
        topo: &'a Topology,
    ) -> impl Iterator<Item = impl Iterator<Item = ((SpaceCoord, TimeCoord), RegionCoords)>> + 'a
    {
        self.arq.segments().enumerate().map(move |(ix, x)| {
            topo.telescoping_times(self.max_time)
                .into_iter()
                .enumerate()
                .map(move |(it, t)| {
                    (
                        (SpaceCoord::from(ix as u32), TimeCoord::from(it as u32)),
                        RegionCoords::new(x, t),
                    )
                })
        })
    }

    pub fn empty() -> Self {
        Self {
            max_time: Timestamp::from_micros(0),
            arq: ArqBounds::empty(11),
        }
    }
}

/// The generic definition of a set of Regions.
/// The current representation is very specific to our current algorithm,
/// but this is an enum to make room for a more generic representation, e.g.
/// a simple Vec<Region>, if we want a more intricate algorithm later.
#[derive(Debug, derive_more::From)]
pub enum RegionSetImpl<T: TreeDataConstraints> {
    /// eXponential Time, Constant Space.
    Xtcs(RegionSetImplXtcs<T>),
}

/// Implementation for the compact XTCS region set format which gets sent over the wire.
/// The coordinates for the regions are specified by a few values.
/// The data to match the coordinates are specified in a 2D vector which must
/// correspond to the generated coordinates.
#[derive(Debug)]
pub struct RegionSetImplXtcs<T: TreeDataConstraints> {
    /// The generator for the coordinates
    pub(super) coords: RegionCoordSetXtcs,

    /// The outer vec corresponds to the spatial segments;
    /// the inner vecs are the time segments.
    pub(super) data: Vec<Vec<T>>,
}

impl<T: TreeDataConstraints> RegionSetImplXtcs<T> {
    pub fn empty() -> Self {
        Self {
            coords: RegionCoordSetXtcs::empty(),
            data: vec![],
        }
    }

    pub fn count(&self) -> usize {
        if self.data.is_empty() {
            0
        } else {
            self.data.len() * self.data[0].len()
        }
    }

    pub fn regions<'a>(&'a self, topo: &'a Topology) -> impl Iterator<Item = RegionImpl<T>> + 'a {
        self.coords.region_coords(topo).map(|((ix, it), coords)| {
            RegionImpl::new(coords, self.data[*ix as usize][*it as usize])
        })
    }

    pub fn diff(&self, other: &Self) -> Self {
        todo!()
    }
}

impl<T: TreeDataConstraints> RegionSetImpl<T> {
    pub fn count(&self) -> usize {
        match self {
            Self::Xtcs(set) => set.count(),
        }
    }

    /// Find a set of Regions which represents the intersection of the two
    /// input RegionSets.
    pub fn diff(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Xtcs(left), Self::Xtcs(right)) => left.diff(right).into(),
        }
        // Notes on a generic algorithm for the diff of generic regions:
        // can we use a Fenwick tree to look up regions?
        // idea:
        // sort the regions by power (problem, there are two power)
        // lookup the region to see if there's already a direct hit (most efficient if the sorting guarantees that larger regions get looked up later)
        // PROBLEM: we *can't* resolve rectangles where one is not a subset of the other
    }
}

pub type RegionSet = RegionSetImpl<RegionData>;
pub type RegionSetXtcs = RegionSetImplXtcs<RegionData>;

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
