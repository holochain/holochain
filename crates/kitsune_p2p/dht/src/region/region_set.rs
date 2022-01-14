use std::{cmp::Ordering, collections::HashMap};

use kitsune_p2p_dht_arc::ArcInterval;
use kitsune_p2p_timestamp::Timestamp;

use crate::{arq::*, coords::*, host::AccessOpStore, op::OpRegion, tree::TreeDataConstraints};

use super::{Region, RegionCoords, RegionData};

#[derive(Debug, derive_more::Constructor)]
pub struct RegionCoordSetXtcs {
    max_time: Timestamp,
    arq_set: ArqSet,
}

impl RegionCoordSetXtcs {
    /// Generate the XTCS region coords given the generating parameters.
    /// Each RegionCoords is paired with the relative spacetime coords, which
    /// can be used to pair the generated coords with stored data.
    pub fn region_coords_flat<'a>(
        &'a self,
        topo: &'a Topology,
    ) -> impl Iterator<Item = ((SpaceCoord, TimeCoord), RegionCoords)> + 'a {
        self.region_coords_nested(topo).flatten()
    }

    pub fn region_coords_nested<'a>(
        &'a self,
        topo: &'a Topology,
    ) -> impl Iterator<Item = impl Iterator<Item = ((SpaceCoord, TimeCoord), RegionCoords)>> + 'a
    {
        self.arq_set.arqs().iter().flat_map(move |arq| {
            arq.segments().enumerate().map(move |(ix, x)| {
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
        })
    }

    pub fn empty() -> Self {
        Self {
            max_time: Timestamp::from_micros(0),
            arq_set: ArqSet::empty(11),
        }
    }
}

/// The generic definition of a set of Regions.
/// The current representation is very specific to our current algorithm,
/// but this is an enum to make room for a more generic representation, e.g.
/// a simple Vec<Region>, if we want a more intricate algorithm later.
#[derive(Debug, derive_more::From)]
pub enum RegionSet<T: TreeDataConstraints = RegionData> {
    /// eXponential Time, Constant Space.
    Xtcs(RegionSetXtcs<T>),
}

/// Implementation for the compact XTCS region set format which gets sent over the wire.
/// The coordinates for the regions are specified by a few values.
/// The data to match the coordinates are specified in a 2D vector which must
/// correspond to the generated coordinates.
#[derive(Debug)]
pub struct RegionSetXtcs<D: TreeDataConstraints = RegionData> {
    /// The generator for the coordinates
    pub(crate) coords: RegionCoordSetXtcs,

    /// The outer vec corresponds to the spatial segments;
    /// the inner vecs are the time segments.
    pub(crate) data: Vec<Vec<D>>,
}

impl<D: TreeDataConstraints> RegionSetXtcs<D> {
    pub fn empty() -> Self {
        Self {
            coords: RegionCoordSetXtcs::empty(),
            data: vec![],
        }
    }

    pub fn new<O: OpRegion<D>, S: AccessOpStore<D, O>>(
        topo: &Topology,
        store: &S,
        coords: RegionCoordSetXtcs,
    ) -> Self {
        let data = coords
            .region_coords_nested(topo)
            .map(|columns| {
                columns
                    .map(|(_, coords)| store.query_region_data(&coords.to_bounds()))
                    .collect()
            })
            .collect();
        Self { coords, data }
    }

    pub fn count(&self) -> usize {
        if self.data.is_empty() {
            0
        } else {
            self.data.len() * self.data[0].len()
        }
    }

    pub fn regions<'a>(&'a self, topo: &'a Topology) -> impl Iterator<Item = Region<D>> + 'a {
        self.coords
            .region_coords_flat(topo)
            .map(|((ix, it), coords)| Region::new(coords, self.data[*ix as usize][*it as usize]))
    }

    /// Reshape the two region sets so that both match, omitting or merging
    /// regions as needed
    pub fn rectify(&mut self, other: &mut Self) {
        debug_assert_eq!(
            self.coords.arq_set, other.coords.arq_set,
            "Currently, different ArqSets are not supported."
        );
        let (a, b, swap) = match self.coords.max_time.cmp(&other.coords.max_time) {
            Ordering::Equal => return,
            Ordering::Less => (self, other, false),
            Ordering::Greater => (other, self, true),
        };
        todo!()
    }

    pub fn diff(&self, other: &Self) -> Self {
        let mut a = self.to_owned();
        let mut b = other.to_owned();
        // a.rectify(&mut b);
        todo!()
    }
}

impl<T: TreeDataConstraints> RegionSet<T> {
    pub fn count(&self) -> usize {
        match self {
            Self::Xtcs(set) => set.count(),
        }
    }
    /// can be used to pair the generated coords with stored data.
    pub fn region_coords<'a>(
        &'a self,
        topo: &'a Topology,
    ) -> impl Iterator<Item = RegionCoords> + 'a {
        match self {
            Self::Xtcs(set) => set
                .coords
                .region_coords_flat(topo)
                .map(|(_, coords)| coords),
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

#[cfg(test)]
mod tests {
    use crate::{op::OpData, test_utils::op_store::OpStore};

    use super::*;

    #[test]
    fn test_regions() {
        // (-512, 512)
        let arq = Arq::new(0.into(), 8, 4).to_bounds();
        assert_eq!(arq.left() as i32, -512);
        assert_eq!(arq.right(), 511 as u32);

        let topo = Topology::identity(Timestamp::from_micros(1000));
        let mut store = OpStore::new(topo.clone());

        // Create a nx by nt grid of ops and integrate into the store
        let nx = 8;
        let nt = 10;
        let ops: Vec<_> = (-1024..1024 as i32)
            .step_by(2056 / nx)
            .flat_map(move |x| {
                (1000..11000 as i64).step_by(10000 / nt).map(move |t| {
                    // 16 x 100 total ops.
                    // x interval: [-1024, -1024)
                    // t interval: [1000, 11000)
                    OpData::fake(x as u32, t, 10)
                })
            })
            .collect();
        assert_eq!(ops.len(), nx * nt);
        store.integrate_ops(ops.into_iter());

        // Calculate region data for all ops.
        // The total count should be half of what's in the op store,
        // since the arq covers exactly half of the ops
        let coords = RegionCoordSetXtcs::new(Timestamp::from_micros(11000), ArqSet::single(arq));
        let rset = RegionSetXtcs::new(&topo, &store, coords);
        assert_eq!(
            rset.data.concat().iter().map(|r| r.count).sum::<u32>() as usize,
            nx * nt / 2
        );
    }

    #[test]
    fn test_rectify() {

        // let coords = [
        //     RegionCoords::new(space, time)
        // ]
        // let a = RegionSetImpl::new()
    }
}
